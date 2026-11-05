use cassis_core::network::NetworkId;
use cassis_core::routing::{
    fetch_announcements, RouteAnnouncement, DEFAULT_NOSTR_RELAYS, TEST_NOSTR_RELAYS,
};
use leptos::prelude::*;
use std::collections::BTreeSet;

/// Parameters naming a test chain: `liquid::testnet`, `arkade::mutinynet`...
const TEST_CHAINS: [&str; 4] = ["testnet", "mutinynet", "signet", "regtest"];

/// Which money the routes shown move.
#[derive(Clone, Copy, PartialEq)]
enum Money {
    Real,
    Test,
    Any,
}

/// A predefined relay set and money filter.
#[derive(Clone, Copy, PartialEq)]
enum Preset {
    Main,
    Test,
}

impl Preset {
    const ALL: [Preset; 2] = [Preset::Main, Preset::Test];

    fn label(self) -> &'static str {
        match self {
            Preset::Main => "MAIN",
            Preset::Test => "TEST",
        }
    }

    fn relays(self) -> BTreeSet<String> {
        let relays = match self {
            Preset::Main => DEFAULT_NOSTR_RELAYS,
            Preset::Test => TEST_NOSTR_RELAYS,
        };
        relays.iter().map(|r| r.to_string()).collect()
    }

    fn money(self) -> Money {
        match self {
            Preset::Main => Money::Real,
            Preset::Test => Money::Test,
        }
    }
}

fn is_test_network(network: &NetworkId) -> bool {
    network.param().is_some_and(|p| TEST_CHAINS.contains(&p))
}

/// Tagged `testmoney` by its router, or bridging a test chain.
fn is_test_route(a: &RouteAnnouncement) -> bool {
    a.test_money || is_test_network(&a.from) || is_test_network(&a.to)
}

#[derive(Clone, PartialEq)]
struct NodeRow {
    pubkey: String,
    iroh: String,
    iroh_relay: Option<String>,
    from: String,
    to: String,
    fee_base_msat: u64,
    fee_ppm: u64,
    incoming_delta_secs: u64,
    transit_slack_secs: u64,
    max_incoming_lifetime_secs: Option<u64>,
    test: bool,
}

impl From<&RouteAnnouncement> for NodeRow {
    fn from(a: &RouteAnnouncement) -> Self {
        NodeRow {
            pubkey: a.node_pubkey.to_hex(),
            iroh: a.iroh_peer_id.clone(),
            iroh_relay: a.iroh_relay.clone(),
            from: a.from.0.clone(),
            to: a.to.0.clone(),
            fee_base_msat: a.fee_base_msat,
            fee_ppm: a.fee_ppm,
            incoming_delta_secs: a.incoming_delta_secs,
            transit_slack_secs: a.transit_slack_secs,
            max_incoming_lifetime_secs: a.max_incoming_lifetime_secs,
            test: is_test_route(a),
        }
    }
}

fn with_ws_scheme(relay: &str) -> String {
    let relay = relay.trim().trim_end_matches('/');
    if relay.starts_with("ws://") || relay.starts_with("wss://") {
        relay.to_string()
    } else {
        format!("wss://{relay}")
    }
}

#[component]
fn App() -> impl IntoView {
    let relays = RwSignal::new(Preset::Main.relays());
    let money = RwSignal::new(Preset::Main.money());
    // Relays offered in the picker: the presets' plus any added by hand.
    let known = RwSignal::new(
        Preset::ALL
            .iter()
            .flat_map(|p| p.relays())
            .collect::<BTreeSet<_>>(),
    );
    let custom = RwSignal::new(String::new());
    // Networks the user switched off.
    let hidden = RwSignal::new(BTreeSet::<String>::new());

    let preset = Memo::new(move |_| {
        Preset::ALL
            .into_iter()
            .find(|p| relays.with(|r| *r == p.relays()) && money.get() == p.money())
    });
    let apply = move |p: Preset| {
        relays.set(p.relays());
        money.set(p.money());
    };

    let resource = LocalResource::new(move || {
        let relays: Vec<String> = relays.get().into_iter().collect();
        async move {
            if relays.is_empty() {
                return Err("pick at least one relay".to_string());
            }
            fetch_announcements(&relays).await
        }
    });

    let announcements = Memo::new(move |_| resource.get().and_then(|r| r.ok()).unwrap_or_default());
    let error = Memo::new(move |_| resource.get().and_then(|r| r.err()));

    // Announcements matching the money filter, before the network one.
    let by_money = Memo::new(move |_| {
        let money = money.get();
        announcements.with(|all| {
            all.iter()
                .filter(|a| match money {
                    Money::Real => !is_test_route(a),
                    Money::Test => is_test_route(a),
                    Money::Any => true,
                })
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    let networks = Memo::new(move |_| {
        by_money.with(|all| {
            all.iter()
                .flat_map(|a| [a.from.0.clone(), a.to.0.clone()])
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        })
    });
    let rows = Memo::new(move |_| {
        let hidden = hidden.get();
        by_money.with(|all| {
            all.iter()
                .filter(|a| !hidden.contains(&a.from.0) && !hidden.contains(&a.to.0))
                .map(NodeRow::from)
                .collect::<Vec<_>>()
        })
    });
    let total = move || announcements.with(Vec::len);

    let toggle_relay = move |relay: String| {
        relays.update(|r| {
            if !r.remove(&relay) {
                r.insert(relay);
            }
        })
    };
    let add_relay = move || {
        let value = custom.get();
        if value.trim().is_empty() {
            return;
        }
        let relay = with_ws_scheme(&value);
        known.update(|k| {
            k.insert(relay.clone());
        });
        relays.update(|r| {
            r.insert(relay);
        });
        custom.set(String::new());
    };
    let toggle_network = move |network: String| {
        hidden.update(|h| {
            if !h.remove(&network) {
                h.insert(network);
            }
        })
    };

    view! {
        <div class="container">
            <header>
                <h1>"cassis explorer"</h1>
                <p>"routing nodes and available networks"</p>
            </header>

            <section class="controls">
                <div class="control-row">
                    <span class="label">"preset"</span>
                    <div class="segmented">
                        {Preset::ALL
                            .into_iter()
                            .map(|p| {
                                view! {
                                    <button
                                        class:active=move || preset.get() == Some(p)
                                        on:click=move |_| apply(p)
                                    >
                                        {p.label()}
                                    </button>
                                }
                            })
                            .collect_view()}
                        <button class="custom-indicator" class:active=move || preset.get().is_none() disabled>
                            "custom"
                        </button>
                    </div>
                </div>

                <div class="control-row">
                    <span class="label">"relays"</span>
                    <div class="chips">
                        <For
                            each=move || known.get()
                            key=|r| r.clone()
                            children=move |r: String| {
                                let checked = {
                                    let r = r.clone();
                                    move || relays.with(|set| set.contains(&r))
                                };
                                let label = r.trim_start_matches("wss://").to_string();
                                view! {
                                    <button
                                        class="chip toggle"
                                        class:on=checked
                                        title={r.clone()}
                                        on:click=move |_| toggle_relay(r.clone())
                                    >
                                        {label}
                                    </button>
                                }
                            }
                        />
                    </div>
                </div>
                <div class="control-row">
                    <span class="label"></span>
                    <form class="add-relay" on:submit=move |ev| {
                        ev.prevent_default();
                        add_relay();
                    }>
                        <input
                            type="text"
                            placeholder="relay.example.com"
                            prop:value=move || custom.get()
                            on:input=move |ev| custom.set(event_target_value(&ev))
                        />
                        <button type="submit">"add relay"</button>
                    </form>
                </div>

                <div class="control-row">
                    <span class="label">"money"</span>
                    <div class="segmented">
                        {[(Money::Real, "real"), (Money::Test, "test"), (Money::Any, "any")]
                            .into_iter()
                            .map(|(m, label)| {
                                view! {
                                    <button
                                        class:active=move || money.get() == m
                                        on:click=move |_| money.set(m)
                                    >
                                        {label}
                                    </button>
                                }
                            })
                            .collect_view()}
                    </div>
                </div>
            </section>

            <Show when=move || error.get().is_some() fallback=|| ()>
                <p class="error">
                    {move || format!("failed to fetch announcements: {}", error.get().unwrap_or_default())}
                </p>
            </Show>

            <section>
                <h2>"networks"</h2>
                <Suspense fallback=move || view! { <p>"loading…"</p> }>
                    <p class="hint">
                        {move || {
                            if networks.with(Vec::is_empty) {
                                "no networks announced on these relays".to_string()
                            } else {
                                "click a network to hide routes touching it".to_string()
                            }
                        }}
                    </p>
                    <div class="chips">
                        <For
                            each=move || networks.get()
                            key=|n| n.clone()
                            children=move |n: String| {
                                let shown = {
                                    let n = n.clone();
                                    move || hidden.with(|h| !h.contains(&n))
                                };
                                let test = NetworkId::parse(&n).is_ok_and(|id| is_test_network(&id));
                                view! {
                                    <button
                                        class="chip toggle"
                                        class:on=shown
                                        class:test=test
                                        title={n.clone()}
                                        on:click=move |_| toggle_network(n.clone())
                                    >
                                        {n.clone()}
                                    </button>
                                }
                            }
                        />
                    </div>
                </Suspense>
            </section>

            <section>
                <h2>
                    "routes "
                    <span class="count">
                        {move || format!("{} shown of {}", rows.with(Vec::len), total())}
                    </span>
                </h2>
                <Suspense fallback=move || view! { <p>"loading…"</p> }>
                    <div class="table-wrap">
                        <table>
                            <thead>
                                <tr>
                                    <th>"node"</th>
                                    <th>"from"</th>
                                    <th>"to"</th>
                                    <th class="num">"base fee (msat)"</th>
                                    <th class="num">"fee ppm"</th>
                                    <th class="num">"incoming delta (s)"</th>
                                    <th class="num">"transit slack (s)"</th>
                                    <th class="num">"max lifetime (s)"</th>
                                </tr>
                            </thead>
                            <tbody>
                                <For
                                    each=move || rows.get()
                                    key=|r| format!("{}-{}->{}", r.pubkey, r.from, r.to)
                                    children=move |r: NodeRow| {
                                        let iroh = match &r.iroh_relay {
                                            Some(relay) => format!("iroh {} via {relay}", r.iroh),
                                            None => format!("iroh {}", r.iroh),
                                        };
                                        view! {
                                            <tr>
                                                <td class="mono" title={iroh}>
                                                    {r.pubkey}
                                                    <Show when=move || r.test fallback=|| ()>
                                                        <span class="badge">"test"</span>
                                                    </Show>
                                                </td>
                                                <td>{r.from}</td>
                                                <td>{r.to}</td>
                                                <td class="num">{r.fee_base_msat}</td>
                                                <td class="num">{r.fee_ppm}</td>
                                                <td class="num">{r.incoming_delta_secs}</td>
                                                <td class="num">{r.transit_slack_secs}</td>
                                                <td class="num">
                                                    {r.max_incoming_lifetime_secs
                                                        .map_or("—".to_string(), |s| s.to_string())}
                                                </td>
                                            </tr>
                                        }
                                    }
                                />
                            </tbody>
                        </table>
                    </div>
                </Suspense>
            </section>
        </div>
    }
}

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| view! { <App /> });
}

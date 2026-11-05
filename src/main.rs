use cassis_core::network::NetworkId;
use cassis_core::routing::{
    fetch_announcements, RouteAnnouncement, DEFAULT_NOSTR_RELAYS, TEST_NOSTR_RELAYS,
};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Parameters naming a test chain: `liquid::testnet`, `arkade::mutinynet`...
const TEST_CHAINS: [&str; 4] = ["testnet", "mutinynet", "signet", "regtest"];

/// localStorage key of the last used [`Settings`].
const SETTINGS_KEY: &str = "cassis-explorer.settings";

/// Which money the routes shown move.
#[derive(Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
enum Money {
    #[default]
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

/// Filtering settings, persisted across visits.
#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    relays: BTreeSet<String>,
    /// Relays added by hand, offered next to the presets' ones.
    added_relays: BTreeSet<String>,
    money: Money,
    hidden_networks: BTreeSet<String>,
    customizing: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            relays: Preset::Main.relays(),
            added_relays: BTreeSet::new(),
            money: Preset::Main.money(),
            hidden_networks: BTreeSet::new(),
            customizing: false,
        }
    }
}

impl Settings {
    fn storage() -> Option<web_sys::Storage> {
        window().local_storage().ok().flatten()
    }

    fn load() -> Self {
        Self::storage()
            .and_then(|s| s.get_item(SETTINGS_KEY).ok().flatten())
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        if let (Some(storage), Ok(json)) = (Self::storage(), serde_json::to_string(self)) {
            let _ = storage.set_item(SETTINGS_KEY, &json);
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
    tagged_test: bool,
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
            tagged_test: a.test_money,
            test: is_test_route(a),
        }
    }
}

impl NodeRow {
    fn key(&self) -> String {
        format!("{}-{}->{}", self.pubkey, self.from, self.to)
    }

    fn max_lifetime(&self) -> String {
        self.max_incoming_lifetime_secs
            .map_or("—".to_string(), |s| s.to_string())
    }

    fn money(&self) -> &'static str {
        match (self.test, self.tagged_test) {
            (false, _) => "real",
            (true, true) => "test (tagged testmoney)",
            (true, false) => "test (test chain)",
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

/// Every field of a route, untruncated.
#[component]
fn RouteDetails(row: NodeRow) -> impl IntoView {
    let fields = [
        ("node pubkey", row.pubkey.clone()),
        ("iroh peer id", row.iroh.clone()),
        ("iroh relay", row.iroh_relay.clone().unwrap_or("—".into())),
        ("from", row.from.clone()),
        ("to", row.to.clone()),
        ("base fee (msat)", row.fee_base_msat.to_string()),
        ("fee ppm", row.fee_ppm.to_string()),
        ("incoming delta (s)", row.incoming_delta_secs.to_string()),
        ("transit slack (s)", row.transit_slack_secs.to_string()),
        ("max lifetime (s)", row.max_lifetime()),
        ("money", row.money().to_string()),
    ];
    view! {
        <dl class="details">
            {fields
                .into_iter()
                .map(|(name, value)| view! { <dt>{name}</dt><dd>{value}</dd> })
                .collect_view()}
        </dl>
    }
}

#[component]
fn App() -> impl IntoView {
    let settings = Settings::load();
    let relays = RwSignal::new(settings.relays);
    let added = RwSignal::new(settings.added_relays);
    let money = RwSignal::new(settings.money);
    // Networks the user switched off.
    let hidden = RwSignal::new(settings.hidden_networks);
    let customizing = RwSignal::new(settings.customizing);
    let custom = RwSignal::new(String::new());
    // The route open in the modal (wide screens).
    let selected = RwSignal::new(None::<NodeRow>);

    Effect::new(move |_| {
        Settings {
            relays: relays.get(),
            added_relays: added.get(),
            money: money.get(),
            hidden_networks: hidden.get(),
            customizing: customizing.get(),
        }
        .save()
    });
    let _ = window_event_listener(leptos::ev::keydown, move |ev| {
        if ev.key() == "Escape" {
            selected.set(None);
        }
    });

    // Relays offered in the picker: the presets' plus any added by hand.
    let known = Memo::new(move |_| {
        let mut known: BTreeSet<String> = Preset::ALL.iter().flat_map(|p| p.relays()).collect();
        known.extend(added.get());
        known.extend(relays.get());
        known
    });
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
    let hidden_count = move || {
        let hidden = hidden.get();
        networks.with(|n| n.iter().filter(|n| hidden.contains(*n)).count())
    };

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
        added.update(|a| {
            a.insert(relay.clone());
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
                        <Show when=move || preset.get().is_none() fallback=|| ()>
                            <button class="custom-indicator active" disabled>
                                "custom"
                            </button>
                        </Show>
                    </div>
                    <button
                        class="customize"
                        class:open=move || customizing.get()
                        on:click=move |_| customizing.update(|c| *c = !*c)
                    >
                        {move || if customizing.get() { "hide filters ▴" } else { "customize ▾" }}
                    </button>
                    <Show when=move || { !customizing.get() && hidden_count() > 0 } fallback=|| ()>
                        <span class="hint">
                            {move || match hidden_count() {
                                1 => "1 network hidden".to_string(),
                                n => format!("{n} networks hidden"),
                            }}
                        </span>
                    </Show>
                </div>

                <Show when=move || customizing.get() fallback=|| ()>
                    <div class="custom-panel">
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
                                                title=r.clone()
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
                            <form
                                class="add-relay"
                                on:submit=move |ev| {
                                    ev.prevent_default();
                                    add_relay();
                                }
                            >
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

                        <div class="control-row">
                            <span class="label">"networks"</span>
                            <Suspense fallback=move || view! { <span class="hint">"loading…"</span> }>
                                <Show
                                    when=move || !networks.with(Vec::is_empty)
                                    fallback=|| view! { <span class="hint">"none announced on these relays"</span> }
                                >
                                    <div class="chips">
                                        <For
                                            each=move || networks.get()
                                            key=|n| n.clone()
                                            children=move |n: String| {
                                                let shown = {
                                                    let n = n.clone();
                                                    move || hidden.with(|h| !h.contains(&n))
                                                };
                                                let test = NetworkId::parse(&n)
                                                    .is_ok_and(|id| is_test_network(&id));
                                                view! {
                                                    <button
                                                        class="chip toggle"
                                                        class:on=shown
                                                        class:test=test
                                                        title=n.clone()
                                                        on:click=move |_| toggle_network(n.clone())
                                                    >
                                                        {n.clone()}
                                                    </button>
                                                }
                                            }
                                        />
                                    </div>
                                </Show>
                            </Suspense>
                        </div>
                    </div>
                </Show>
            </section>

            <Show when=move || error.get().is_some() fallback=|| ()>
                <p class="error">
                    {move || {
                        format!("failed to fetch announcements: {}", error.get().unwrap_or_default())
                    }}
                </p>
            </Show>

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
                                    <th class="num" title="base fee (msat)">"base (msat)"</th>
                                    <th class="num" title="proportional fee (ppm)">"ppm"</th>
                                    <th class="num" title="incoming delta (s)">"delta (s)"</th>
                                    <th class="num" title="transit slack (s)">"slack (s)"</th>
                                    <th class="num" title="max incoming lifetime (s)">"max life (s)"</th>
                                </tr>
                            </thead>
                            <tbody>
                                <For
                                    each=move || rows.get()
                                    key=NodeRow::key
                                    children=move |r: NodeRow| {
                                        let open = r.clone();
                                        view! {
                                            <tr on:click=move |_| selected.set(Some(open.clone()))>
                                                <td>
                                                    <div class="cell mono" title=r.pubkey.clone()>
                                                        {r.pubkey.clone()}
                                                    </div>
                                                    <Show when=move || r.test fallback=|| ()>
                                                        <span class="badge">"test"</span>
                                                    </Show>
                                                </td>
                                                <td>
                                                    <div class="cell" title=r.from.clone()>
                                                        {r.from.clone()}
                                                    </div>
                                                </td>
                                                <td>
                                                    <div class="cell" title=r.to.clone()>
                                                        {r.to.clone()}
                                                    </div>
                                                </td>
                                                <td class="num">{r.fee_base_msat}</td>
                                                <td class="num">{r.fee_ppm}</td>
                                                <td class="num">{r.incoming_delta_secs}</td>
                                                <td class="num">{r.transit_slack_secs}</td>
                                                <td class="num">{r.max_lifetime()}</td>
                                            </tr>
                                        }
                                    }
                                />
                            </tbody>
                        </table>
                    </div>

                    <div class="cards">
                        <For
                            each=move || rows.get()
                            key=NodeRow::key
                            children=move |r: NodeRow| {
                                let expanded = RwSignal::new(false);
                                let details = r.clone();
                                view! {
                                    <div
                                        class="card"
                                        class:expanded=move || expanded.get()
                                        on:click=move |_| expanded.update(|e| *e = !*e)
                                    >
                                        <Show
                                            when=move || expanded.get()
                                            fallback=move || {
                                                let r = r.clone();
                                                view! {
                                                    <div class="card-route">
                                                        <span class="cell">{r.from.clone()}</span>
                                                        <span class="arrow">"→"</span>
                                                        <span class="cell">{r.to.clone()}</span>
                                                        <Show when=move || r.test fallback=|| ()>
                                                            <span class="badge">"test"</span>
                                                        </Show>
                                                    </div>
                                                    <div class="cell mono dim">{r.pubkey.clone()}</div>
                                                    <div class="card-fees">
                                                        {format!(
                                                            "{} msat + {} ppm · Δ {}s · slack {}s",
                                                            r.fee_base_msat,
                                                            r.fee_ppm,
                                                            r.incoming_delta_secs,
                                                            r.transit_slack_secs,
                                                        )}
                                                    </div>
                                                }
                                            }
                                        >
                                            <RouteDetails row=details.clone() />
                                        </Show>
                                    </div>
                                }
                            }
                        />
                    </div>
                </Suspense>
            </section>

            {move || {
                selected
                    .get()
                    .map(|row| {
                        view! {
                            <div class="modal-backdrop" on:click=move |_| selected.set(None)>
                                <div class="modal" on:click=|ev| ev.stop_propagation()>
                                    <div class="modal-head">
                                        <h3>{format!("{} → {}", row.from, row.to)}</h3>
                                        <button class="close" on:click=move |_| selected.set(None)>
                                            "×"
                                        </button>
                                    </div>
                                    <RouteDetails row=row.clone() />
                                </div>
                            </div>
                        }
                    })
            }}
        </div>
    }
}

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| view! { <App /> });
}

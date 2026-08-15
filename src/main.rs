use cassis_core::RouteAnnouncement;
use cassis_routing::fetch_announcements;
use leptos::prelude::*;

const RELAYS: [&str; 3] = ["wss://nos.lol", "wss://nostr.mom", "wss://relay.damus.io"];

#[derive(Clone, PartialEq)]
struct NodeRow {
    pubkey: String,
    iroh: String,
    from: String,
    to: String,
    fee_base_msat: u64,
    fee_ppm: u64,
    incoming_delta_secs: u64,
    transit_slack_secs: u64,
    relays: String,
}

fn to_rows(announcements: &[RouteAnnouncement]) -> Vec<NodeRow> {
    announcements
        .iter()
        .map(|a| NodeRow {
            pubkey: a.node_pubkey.to_hex(),
            iroh: a.iroh_peer_id.clone(),
            from: a.from.0.clone(),
            to: a.to.0.clone(),
            fee_base_msat: a.fee_base_msat,
            fee_ppm: a.fee_ppm,
            incoming_delta_secs: a.incoming_delta_secs,
            transit_slack_secs: a.transit_slack_secs,
            relays: a.relays.join(", "),
        })
        .collect()
}

fn available_networks(announcements: &[RouteAnnouncement]) -> Vec<String> {
    let mut nets: Vec<String> = announcements
        .iter()
        .flat_map(|a| [a.from.0.clone(), a.to.0.clone()])
        .collect();
    nets.sort_unstable();
    nets.dedup();
    nets
}

fn with_ws_scheme(relay: &str) -> String {
    let relay = relay.trim();
    if relay.starts_with("ws://") || relay.starts_with("wss://") {
        relay.to_string()
    } else {
        format!("wss://{relay}")
    }
}

#[component]
fn App() -> impl IntoView {
    let relay = RwSignal::new(RELAYS[0].to_string());
    let custom = RwSignal::new(String::new());

    let resource = LocalResource::new(move || {
        let relay = relay.get();
        async move {
            let relays = vec![with_ws_scheme(&relay)];
            fetch_announcements(&relays).await.map_err(|e| e.to_string())
        }
    });

    let rows = Memo::new(move |_| {
        resource
            .get()
            .and_then(|r| r.ok())
            .map(|a| to_rows(&a))
            .unwrap_or_default()
    });
    let networks = Memo::new(move |_| {
        resource
            .get()
            .and_then(|r| r.ok())
            .map(|a| available_networks(&a))
            .unwrap_or_default()
    });
    let error = Memo::new(move |_| resource.get().and_then(|r| r.err()));

    let select_options = Memo::new(move |_| {
        let mut opts: Vec<String> = RELAYS.iter().map(|s| s.to_string()).collect();
        let current = relay.get();
        if !opts.contains(&current) {
            opts.push(current);
        }
        opts
    });

    view! {
        <div class="container">
            <header>
                <h1>"cassis explorer"</h1>
                <p>"routing nodes and available networks"</p>
            </header>

            <section class="relay-picker">
                <label for="relay-select">"relay"</label>
                <select
                    id="relay-select"
                    prop:value=move || relay.get()
                    on:change=move |ev| relay.set(event_target_value(&ev))
                >
                    <For
                        each=move || select_options.get()
                        key=|r| r.clone()
                        children=move |r: String| {
                            view! { <option value={r.clone()}>{r.clone()}</option> }
                        }
                    />
                </select>
                <input
                    type="text"
                    placeholder="relay.example.com"
                    prop:value=move || custom.get()
                    on:input=move |ev| custom.set(event_target_value(&ev))
                />
                <button on:click=move |_| {
                    let value = custom.get();
                    if !value.trim().is_empty() {
                        relay.set(value.trim().to_string());
                    }
                }>"connect"</button>
            </section>

            <Show
                when=move || error.get().is_some()
                fallback=|| ()
            >
                <p class="error">
                    {move || format!("failed to fetch announcements: {}", error.get().unwrap_or_default())}
                </p>
            </Show>

            <section>
                <h2>"available networks"</h2>
                <Suspense fallback=move || view! { <p>"loading…"</p> }>
                    <div class="chips">
                        <For
                            each=move || networks.get()
                            key=|n| n.clone()
                            children=move |n: String| {
                                view! { <span class="chip">{n}</span> }
                            }
                        />
                    </div>
                </Suspense>
            </section>

            <section>
                <h2>"routing nodes"</h2>
                <Suspense fallback=move || view! { <p>"loading…"</p> }>
                    <table>
                        <thead>
                            <tr>
                                <th>"node pubkey"</th>
                                <th>"from"</th>
                                <th>"to"</th>
                                <th>"base fee (msat)"</th>
                                <th>"fee ppm"</th>
                                <th>"incoming delta (s)"</th>
                                <th>"transit slack (s)"</th>
                                <th>"relays"</th>
                            </tr>
                        </thead>
                        <tbody>
                            <For
                                each=move || rows.get()
                                key=|r| format!("{}-{}", r.pubkey, r.from)
                                children=move |r: NodeRow| {
                                    view! {
                                        <tr>
                                            <td class="mono">{r.pubkey}</td>
                                            <td>{r.from}</td>
                                            <td>{r.to}</td>
                                            <td class="num">{r.fee_base_msat}</td>
                                            <td class="num">{r.fee_ppm}</td>
                                            <td class="num">{r.incoming_delta_secs}</td>
                                            <td class="num">{r.transit_slack_secs}</td>
                                            <td class="mono">{r.relays}</td>
                                        </tr>
                                    }
                                }
                            />
                        </tbody>
                    </table>
                </Suspense>
            </section>
        </div>
    }
}

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| view! { <App /> });
}

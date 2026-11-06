use cassis_core::network::NetworkId;
use cassis_core::routing::{
    fetch_announcements, RouteAnnouncement, DEFAULT_NOSTR_RELAYS, TEST_NOSTR_RELAYS,
};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

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
    autoname: String,
    /// The node's color, from its key's autohue.
    color: String,
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
            autoname: a.autoname(),
            color: hue_color(a.autohue()),
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

    /// The unit after [`Self::max_lifetime`], none when unset.
    fn max_lifetime_unit(&self) -> &'static str {
        self.max_incoming_lifetime_secs.map_or("", |_| "s")
    }

    /// Why the route counts as test money, if it does.
    fn test_reason(&self) -> Option<&'static str> {
        match (self.test, self.tagged_test) {
            (false, _) => None,
            (true, true) => Some("tagged testmoney"),
            (true, false) => Some("bridges a test chain"),
        }
    }

    /// The start of the node key, as shown next to its autoname.
    fn short_key(&self) -> String {
        format!("{}…", &self.pubkey[..self.pubkey.len().min(8)])
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

/// Hex color of `hue` (degrees), light enough for the dark background.
fn hue_color(hue: u16) -> String {
    // HSL with saturation 70% and lightness 65%.
    let (s, l) = (0.7, 0.65);
    let c = (1.0 - (2.0 * l - 1.0_f64).abs()) * s;
    let h = f64::from(hue % 360) / 60.0;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u8 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let byte = |v: f64| ((v + m) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", byte(r), byte(g), byte(b))
}

/// Dark tints behind the network boxes, one per network.
const NETWORK_COLORS: [&str; 8] = [
    "#14202e", "#132619", "#272111", "#261421", "#1e1830", "#291515", "#112628", "#25220e",
];

/// Longest network param drawn in the graph; the full name is its tooltip.
const MAX_NETWORK_PARAM: usize = 22;

/// A network name split at `::`: its kind and, if any, its param. Lenient,
/// so names that don't parse still render.
fn network_parts(name: &str) -> (&str, Option<&str>) {
    match name.split_once("::") {
        Some((kind, param)) => (kind, Some(param)),
        None => (name, None),
    }
}

/// The color a network kind is drawn in, the same wherever it shows.
fn kind_color(kind: &str) -> String {
    let hue = match kind {
        "bitcoin" => 30,
        "lightning" => 50,
        "liquid" => 175,
        "cashu" => 280,
        "fedimint" => 215,
        "arkade" => 330,
        "rootstock" => 120,
        // Unknown kinds get a stable hue from their name.
        _ => {
            kind.bytes()
                .fold(0u16, |h, b| h.wrapping_mul(31).wrapping_add(b.into()))
                % 360
        }
    };
    hue_color(hue)
}

#[wasm_bindgen]
extern "C" {
    /// Lays DOT source out into an SVG string, with graphviz (see index.html).
    #[wasm_bindgen(js_name = renderDot, catch)]
    fn render_dot_promise(dot: &str) -> Result<js_sys::Promise, JsValue>;
}

async fn render_dot(dot: String) -> Result<String, String> {
    let promise = render_dot_promise(&dot).map_err(|e| format!("{e:?}"))?;
    let svg = JsFuture::from(promise)
        .await
        .map_err(|e| format!("{e:?}"))?;
    Ok(svg.as_string().unwrap_or_default())
}

/// `s` cut to `max` characters, ending in "…" when cut.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max - 1).chain(['…']).collect()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Graphviz HTML-like label of a network, styled like [`NetworkName`].
fn network_label(name: &str, test: bool) -> String {
    let (kind, param) = network_parts(name);
    let mut label = format!(
        "<<B><FONT COLOR=\"{}\">{}</FONT></B>",
        kind_color(kind),
        html_escape(kind)
    );
    if let Some(param) = param {
        let color = if test { "#d29922" } else { "#8b949e" };
        label += &format!(
            "<FONT COLOR=\"#484f58\">::</FONT><FONT FACE=\"monospace\" COLOR=\"{color}\">{}</FONT>",
            html_escape(&truncate(param, MAX_NETWORK_PARAM))
        );
    }
    label + ">"
}

fn dot_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Graphviz source of `rows`: a box per network holding a circle per router
/// present in it, and an arrow per route between a router's circles. Edge
/// `i` gets the SVG id `route-{i}`, so clicks map back to `rows[i]`.
fn routes_dot(rows: &[NodeRow]) -> String {
    let mut networks: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for r in rows {
        networks.entry(&r.from).or_default().insert(&r.pubkey);
        networks.entry(&r.to).or_default().insert(&r.pubkey);
    }
    // A row per router, to name and color its nodes.
    let routers: BTreeMap<&str, &NodeRow> = rows.iter().map(|r| (r.pubkey.as_str(), r)).collect();
    let router = |pk: &str| routers.keys().position(|k| *k == pk).unwrap_or_default();
    let network = |n: &str| networks.keys().position(|k| *k == n).unwrap_or_default();

    let mut dot = String::from(
        "digraph routes {\n\
         rankdir=LR; bgcolor=transparent; nodesep=0.3; ranksep=1.2;\n\
         fontname=\"system-ui\"; fontcolor=\"#e6edf3\";\n\
         node [shape=box, style=\"rounded,filled\", margin=\"0.16,0.05\", fontsize=9, \
         fontname=\"system-ui\", fontcolor=\"#0d1117\", color=\"#0d1117\"];\n\
         edge [penwidth=1.5, arrowsize=0.7];\n",
    );
    for (i, (name, pubkeys)) in networks.iter().enumerate() {
        let test = NetworkId::parse(name).is_ok_and(|id| is_test_network(&id));
        let style = if test {
            "rounded,filled,dashed"
        } else {
            "rounded,filled"
        };
        dot += &format!(
            "subgraph cluster_{i} {{ label={}; tooltip={}; style=\"{style}\"; \
             fillcolor=\"{}\"; color=\"#30363d\"; margin=24;\n",
            network_label(name, test),
            dot_quote(name),
            NETWORK_COLORS[i % NETWORK_COLORS.len()],
        );
        for pk in pubkeys {
            let node = routers[pk];
            // Two words a line, so the box stays compact.
            let words: Vec<&str> = node.autoname.split('-').collect();
            let label = words
                .chunks(2)
                .map(|w| w.join("-"))
                .collect::<Vec<_>>()
                .join("\n");
            dot += &format!(
                "n{i}_{} [label={}, tooltip={}, fillcolor=\"{}\"];\n",
                router(pk),
                dot_quote(&label),
                dot_quote(&format!("{}\n{pk}", node.autoname)),
                node.color,
            );
        }
        dot += "}\n";
    }
    for (e, r) in rows.iter().enumerate() {
        let j = router(&r.pubkey);
        dot += &format!(
            "n{}_{j} -> n{}_{j} [id=\"route-{e}\", color=\"{}\", tooltip={}];\n",
            network(&r.from),
            network(&r.to),
            r.color,
            dot_quote(&format!(
                "{}: {} → {}, {} msat + {} ppm",
                r.autoname, r.from, r.to, r.fee_base_msat, r.fee_ppm
            )),
        );
    }
    dot + "}\n"
}

/// A node's color swatch and autoname.
#[component]
fn NodeName(row: NodeRow) -> impl IntoView {
    view! {
        <span class="node-name" title=row.pubkey.clone()>
            <span class="swatch" style=format!("background: {}", row.color)></span>
            <span class="cell">{row.autoname.clone()}</span>
        </span>
    }
}

/// A network name, its kind colored and its param muted (amber for a test
/// chain). `class` is added to the outer span, e.g. `cell` to truncate it.
#[component]
fn NetworkName(name: String, #[prop(optional)] class: &'static str) -> impl IntoView {
    let test = NetworkId::parse(&name).is_ok_and(|id| is_test_network(&id));
    let (kind, param) = network_parts(&name);
    let color = format!("color: {}", kind_color(kind));
    let (kind, param) = (kind.to_string(), param.map(str::to_string));
    view! {
        <span class=format!("network-name {class}") class:test=test title=name.clone()>
            <span class="kind" style=color>{kind}</span>
            {param.map(|p| view! { <span class="sep">"::"</span><span class="param">{p}</span> })}
        </span>
    }
}

/// Every field of a route, untruncated.
#[component]
fn RouteDetails(row: NodeRow) -> impl IntoView {
    let stats = [
        ("base fee", row.fee_base_msat.to_string(), "msat", true),
        ("fee rate", row.fee_ppm.to_string(), "ppm", true),
        ("delta", row.incoming_delta_secs.to_string(), "s", false),
        ("slack", row.transit_slack_secs.to_string(), "s", false),
        (
            "max life",
            row.max_lifetime(),
            row.max_lifetime_unit(),
            false,
        ),
    ];
    let connection = [
        ("iroh peer id", row.iroh.clone()),
        ("iroh relay", row.iroh_relay.clone().unwrap_or("—".into())),
    ];
    let test_reason = row.test_reason();
    view! {
        <div class="details">
            <div class="identity">
                <span class="swatch big" style=format!("background: {}", row.color)></span>
                <div class="identity-text">
                    <div class="autoname">{row.autoname.clone()}</div>
                    <div class="mono key">{row.pubkey.clone()}</div>
                </div>
            </div>

            <div class="route-line">
                <NetworkName name=row.from.clone() class="network" />
                <span class="arrow">"→"</span>
                <NetworkName name=row.to.clone() class="network" />
                {test_reason
                    .map(|reason| view! { <span class="badge" title=reason>"test · " {reason}</span> })}
            </div>

            <div class="stats">
                {stats
                    .into_iter()
                    .map(|(name, value, unit, primary)| {
                        view! {
                            <div class="stat" class:primary=primary>
                                <div class="stat-label">{name}</div>
                                <div class="stat-value">
                                    {value} <span class="unit">" " {unit}</span>
                                </div>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>

            <dl class="connection">
                {connection
                    .into_iter()
                    .map(|(name, value)| view! { <dt>{name}</dt><dd class="mono">{value}</dd> })
                    .collect_view()}
            </dl>
        </div>
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
    let dot = Memo::new(move |_| rows.with(|r| routes_dot(r)));
    let graph = LocalResource::new(move || render_dot(dot.get()));
    // Opens the route whose edge was clicked in the graph.
    let open_edge = move |ev: leptos::ev::MouseEvent| {
        let route = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .and_then(|el| el.closest("g.edge").ok().flatten())
            .and_then(|edge| edge.id().strip_prefix("route-")?.parse::<usize>().ok())
            .and_then(|i| rows.with(|r| r.get(i).cloned()));
        if let Some(route) = route {
            selected.set(Some(route));
        }
    };
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
                                                        <NetworkName name=n.clone() />
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
                <h2>"graph"</h2>
                <Suspense fallback=move || view! { <p>"loading…"</p> }>
                    {move || match graph.get() {
                        Some(Err(e)) => {
                            view! { <p class="error">{format!("failed to draw the graph: {e}")}</p> }
                                .into_any()
                        }
                        _ if rows.with(Vec::is_empty) => {
                            view! { <p class="hint">"no routes to draw"</p> }.into_any()
                        }
                        svg => {
                            view! {
                                <div
                                    class="graph"
                                    inner_html=svg.and_then(Result::ok).unwrap_or_default()
                                    on:click=open_edge
                                ></div>
                            }
                                .into_any()
                        }
                    }}
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
                                    <th>"route"</th>
                                    <th class="num" title="base fee (msat) + proportional fee (ppm)">
                                        "fee"
                                    </th>
                                    <th class="num dim-col" title="incoming delta (s)">"delta"</th>
                                    <th class="num dim-col" title="transit slack (s)">"slack"</th>
                                    <th class="num dim-col" title="max incoming lifetime (s)">"max life"</th>
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
                                                    <NodeName row=r.clone() />
                                                    <div class="mono key">{r.short_key()}</div>
                                                </td>
                                                <td>
                                                    <div class="route-cell">
                                                        <NetworkName name=r.from.clone() class="cell" />
                                                        <span class="arrow">"→"</span>
                                                        <NetworkName name=r.to.clone() class="cell" />
                                                        <Show when=move || r.test fallback=|| ()>
                                                            <span class="badge">"test"</span>
                                                        </Show>
                                                    </div>
                                                </td>
                                                <td class="num fee">
                                                    {r.fee_base_msat}
                                                    <span class="unit">" msat"</span>
                                                    <span class="plus">" + "</span>
                                                    {r.fee_ppm}
                                                    <span class="unit">" ppm"</span>
                                                </td>
                                                <td class="num dim-col">{r.incoming_delta_secs} <span class="unit">"s"</span></td>
                                                <td class="num dim-col">{r.transit_slack_secs} <span class="unit">"s"</span></td>
                                                <td class="num dim-col">{r.max_lifetime()} <span class="unit">{r.max_lifetime_unit()}</span></td>
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
                                                        <NetworkName name=r.from.clone() class="cell" />
                                                        <span class="arrow">"→"</span>
                                                        <NetworkName name=r.to.clone() class="cell" />
                                                        <Show when=move || r.test fallback=|| ()>
                                                            <span class="badge">"test"</span>
                                                        </Show>
                                                    </div>
                                                    <NodeName row=r.clone() />
                                                    <div class="card-fees">
                                                        <span class="fee">
                                                            {format!("{} msat + {} ppm", r.fee_base_msat, r.fee_ppm)}
                                                        </span>
                                                        {format!(
                                                            " · Δ {}s · slack {}s",
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
                                    <button class="close" on:click=move |_| selected.set(None)>
                                        "×"
                                    </button>
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

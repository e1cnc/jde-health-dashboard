use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::collections::BTreeMap;
use gloo_timers::callback::Interval;

// --- Data Models ---

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    #[serde(rename = "instanceName")]
    pub instance_name: Option<String>,
    #[serde(rename = "instanceStatus")]
    pub instance_status: Option<String>,
    #[serde(rename = "healthStatus")]
    pub health_status: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct GroupStatus {
    pub name: String,
    pub total: usize,
    pub ok: usize,
    pub err: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CustomerSummary {
    pub name: String,
    pub groups: BTreeMap<String, GroupStatus>,
}

#[derive(Clone, Copy, PartialEq)]
enum Filter {
    All,
    Failed,
    Running,
}

// --- Helper Functions ---

fn parse_meta(filename: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = filename.split('_').collect();
    if parts.len() >= 2 {
        Some((parts[0].to_string(), parts[1].to_uppercase()))
    } else {
        None
    }
}

async fn fetch_all_health_data() -> BTreeMap<String, CustomerSummary> {
    let mut customers: BTreeMap<String, CustomerSummary> = BTreeMap::new();
    let base_url = "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/";

    if let Ok(resp) = Request::get(base_url).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(objects) = json.get("objects").and_then(|o| o.as_array()) {
                for obj in objects {
                    if let Some(name) = obj.get("name").and_then(|n| n.as_str()) {
                        if name.contains("_health.json") || name.ends_with("_latest.json") {
                            if let Some((cust_name, grp_name)) = parse_meta(name) {
                                let file_url = format!("{}{}", base_url, name);
                                if let Ok(file_resp) = Request::get(&file_url).send().await {
                                    if let Ok(instances) = file_resp.json::<Vec<HealthInstance>>().await {
                                        let cust = customers.entry(cust_name.clone()).or_insert(CustomerSummary {
                                            name: cust_name,
                                            groups: BTreeMap::new(),
                                        });

                                        let mut ok = 0;
                                        let mut err = 0;
                                        for inst in &instances {
                                            let status = inst.instance_status.as_deref().unwrap_or("");
                                            let health = inst.health_status.as_deref().unwrap_or("");
                                            if status == "RUNNING" && health == "Passed" {
                                                ok += 1;
                                            } else {
                                                err += 1;
                                            }
                                        }

                                        let group = cust.groups.entry(grp_name.clone()).or_insert(GroupStatus {
                                            name: grp_name,
                                            total: 0,
                                            ok: 0,
                                            err: 0,
                                        });
                                        group.total += instances.len();
                                        group.ok += ok;
                                        group.err += err;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    customers
}

// --- UI Components ---

#[component]
fn App() -> impl IntoView {
    let (refresh_count, set_refresh_count) = create_signal(0);
    let (filter, set_filter) = create_signal(Filter::All);
    
    let health_data = create_resource(move || refresh_count.get(), |_| async move {
        fetch_all_health_data().await
    });

    core::mem::forget(Interval::new(60_000, move || {
        set_refresh_count.update(|n| *n += 1);
    }));

    view! {
        <div style="padding: 20px; background: #f8fafc; min-height: 100vh; font-family: -apple-system, sans-serif;">
            <div style="max-width: 1200px; margin: auto;">
                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 25px;">
                    <h2 style="margin: 0; color: #0f172a; font-weight: 800; letter-spacing: -0.5px;">"JDE HEALTH MONITOR"</h2>
                    
                    <div style="background: #e2e8f0; padding: 3px; border-radius: 10px; display: flex; gap: 2px;">
                        // Added 'move' keyword to the click handlers below
                        <button on:click=move |_| set_filter.set(Filter::All)
                            style=move || format!("border: none; padding: 8px 16px; border-radius: 8px; cursor: pointer; font-weight: 700; font-size: 0.7em; background: {}; color: {}; transition: 0.2s;", 
                                if filter.get() == Filter::All { "white" } else { "transparent" },
                                if filter.get() == Filter::All { "#1e293b" } else { "#64748b" })> "ALL" </button>
                        <button on:click=move |_| set_filter.set(Filter::Failed)
                            style=move || format!("border: none; padding: 8px 16px; border-radius: 8px; cursor: pointer; font-weight: 700; font-size: 0.7em; background: {}; color: {}; transition: 0.2s;", 
                                if filter.get() == Filter::Failed { "#ef4444" } else { "transparent" },
                                if filter.get() == Filter::Failed { "white" } else { "#64748b" })> "FAILED" </button>
                        <button on:click=move |_| set_filter.set(Filter::Running)
                            style=move || format!("border: none; padding: 8px 16px; border-radius: 8px; cursor: pointer; font-weight: 700; font-size: 0.7em; background: {}; color: {}; transition: 0.2s;", 
                                if filter.get() == Filter::Running { "#22c55e" } else { "transparent" },
                                if filter.get() == Filter::Running { "white" } else { "#64748b" })> "HEALTHY" </button>
                    </div>
                </div>
                
                <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 16px;">
                    <Transition fallback=|| view! { <p style="color: #64748b;">"Syncing Live Status..."</p> }>
                        {move || {
                            health_data.get().unwrap_or_default().into_iter()
                                .filter(|(_, cust)| {
                                    let has_err = cust.groups.values().any(|g| g.err > 0);
                                    match filter.get() {
                                        Filter::All => true,
                                        Filter::Failed => has_err,
                                        Filter::Running => !has_err,
                                    }
                                })
                                .map(|(_, cust)| {
                                    let total_ok: usize = cust.groups.values().map(|g| g.ok).sum();
                                    let total_err: usize = cust.groups.values().map(|g| g.err).sum();
                                    let total_inst: usize = cust.groups.values().map(|g| g.total).sum();
                                    let is_critical = total_err > 0;

                                    view! {
                                        <div style=format!("background: white; border-radius: 12px; padding: 18px; box-shadow: 0 4px 6px -1px rgba(0,0,0,0.05); border: 1px solid #e2e8f0; border-top: 5px solid {};", if is_critical { "#ef4444" } else { "#10b981" })>
                                            <div style="display: flex; justify-content: space-between; align-items: start; margin-bottom: 16px;">
                                                <h3 style="margin: 0; font-size: 1.1em; color: #1e293b; font-weight: 800;">{cust.name}</h3>
                                                <div style=format!("font-size: 0.85em; font-weight: 900; color: {};", if is_critical { "#ef4444" } else { "#10b981" })>
                                                    {format!("{}%", if total_inst > 0 { (total_ok as f32 / total_inst as f32 * 100.0) as i32 } else { 0 })}
                                                </div>
                                            </div>

                                            <div style="display: flex; flex-wrap: wrap; gap: 8px; margin-bottom: 18px;">
                                                {cust.groups.values().cloned().map(|g| {
                                                    let group_err = g.err > 0;
                                                    let bg = if group_err { "#fee2e2" } else { "#f1f5f9" };
                                                    let fg = if group_err { "#b91c1c" } else { "#475569" };

                                                    view! {
                                                        <div style=format!("background: {}; color: {}; padding: 4px 10px; border-radius: 6px; font-weight: 800; font-size: 0.72em; display: flex; align-items: center; gap: 6px; border: 1px solid rgba(0,0,0,0.05);", bg, fg)>
                                                            <div style=format!("width: 6px; height: 6px; border-radius: 50%; background: {};", if group_err { "#ef4444" } else { "#22c55e" })></div>
                                                            <span>{format!("{}: {}", g.name, g.total)}</span>
                                                        </div>
                                                    }
                                                }).collect_view()}
                                            </div>

                                            <div style="display: flex; justify-content: space-between; font-size: 0.75em; font-weight: 700; color: #94a3b8; padding-top: 12px; border-top: 1px solid #f1f5f9;">
                                                <span>{format!("{} OK", total_ok)}</span>
                                                <span>{format!("{} ERROR", total_err)}</span>
                                            </div>
                                        </div>
                                    }
                                }).collect_view()
                        }}
                    </Transition>
                </div>
            </div>
        </div>
    }
}

fn main() { mount_to_body(|| view! { <App /> }) }
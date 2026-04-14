use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::collections::BTreeMap;
use gloo_timers::callback::Interval;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    #[serde(rename = "instanceStatus")]
    pub instance_status: Option<String>,
    #[serde(rename = "healthStatus")]
    pub health_status: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct GroupStatus {
    pub name: String,
    pub total: usize,
    pub is_healthy: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CustomerSummary {
    pub name: String,
    pub groups: BTreeMap<String, GroupStatus>,
}

#[derive(Clone, Copy, PartialEq)]
enum Filter { All, Failed, Healthy }

async fn fetch_all_health_data() -> BTreeMap<String, CustomerSummary> {
    let mut customers: BTreeMap<String, CustomerSummary> = BTreeMap::new();
    let mut winning_files: BTreeMap<String, String> = BTreeMap::new();
    let base_url = "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/";

    if let Ok(resp) = Request::get(base_url).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(objects) = json.get("objects").and_then(|o| o.as_array()) {
                for obj in objects {
                    if let Some(name) = obj.get("name").and_then(|n| n.as_str()) {
                        // Skip the generic dashboard file and only process health jsons
                        if name.ends_with(".json") && name != "data.json" {
                            let parts: Vec<&str> = name.split('_').collect();
                            if parts.len() >= 2 {
                                let key = format!("{}_{}", parts[0], parts[1].to_uppercase());
                                
                                // STRIC_PRIORITY: Always prefer files ending in _latest.json
                                let current_winner = winning_files.get(&key);
                                if current_winner.is_none() || name.ends_with("_latest.json") {
                                    winning_files.insert(key, name.to_string());
                                }
                            }
                        }
                    }
                }

                for (_, filename) in winning_files {
                    let file_url = format!("{}{}", base_url, filename);
                    if let Ok(file_resp) = Request::get(&file_url).send().await {
                        if let Ok(instances) = file_resp.json::<Vec<HealthInstance>>().await {
                            let parts: Vec<&str> = filename.split('_').collect();
                            let cust_name = parts[0].to_string();
                            let grp_name = parts[1].to_uppercase();

                            let cust = customers.entry(cust_name.clone()).or_insert(CustomerSummary {
                                name: cust_name,
                                groups: BTreeMap::new(),
                            });

                            let all_ok = instances.iter().all(|inst| {
                                let s = inst.instance_status.as_deref().unwrap_or("").to_uppercase();
                                let h = inst.health_status.as_deref().unwrap_or("").to_lowercase();
                                s == "RUNNING" && h == "passed"
                            });

                            cust.groups.insert(grp_name.clone(), GroupStatus {
                                name: grp_name,
                                total: instances.len(),
                                is_healthy: all_ok,
                            });
                        }
                    }
                }
            }
        }
    }
    customers
}

#[component]
fn App() -> impl IntoView {
    let (refresh_count, set_refresh_count) = create_signal(0);
    let (filter, set_filter) = create_signal(Filter::All);
    let health_data = create_resource(move || refresh_count.get(), |_| async move { fetch_all_health_data().await });

    core::mem::forget(Interval::new(60_000, move || set_refresh_count.update(|n| *n += 1)));

    view! {
        <div style="padding: 20px; background: #f8fafc; min-height: 100vh; font-family: sans-serif;">
            <div style="max-width: 1200px; margin: auto;">
                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 25px;">
                    <h2 style="margin: 0; color: #0f172a; font-weight: 800; letter-spacing: -0.5px;">"JDE HEALTH DASHBOARD"</h2>
                    <div style="background: #e2e8f0; padding: 4px; border-radius: 12px; display: flex; gap: 4px;">
                        <button on:click=move |_| set_filter.set(Filter::All) 
                            style=move || format!("border: none; padding: 8px 16px; border-radius: 8px; cursor: pointer; font-weight: 700; font-size: 0.75em; background: {}; color: {};", 
                                if filter.get() == Filter::All { "white" } else { "transparent" },
                                if filter.get() == Filter::All { "#1e293b" } else { "#64748b" })> "ALL" </button>
                        <button on:click=move |_| set_filter.set(Filter::Failed) 
                            style=move || format!("border: none; padding: 8px 16px; border-radius: 8px; cursor: pointer; font-weight: 700; font-size: 0.75em; background: {}; color: {};", 
                                if filter.get() == Filter::Failed { "#ef4444" } else { "transparent" },
                                if filter.get() == Filter::Failed { "white" } else { "#64748b" })> "FAILED" </button>
                        <button on:click=move |_| set_filter.set(Filter::Healthy) 
                            style=move || format!("border: none; padding: 8px 16px; border-radius: 8px; cursor: pointer; font-weight: 700; font-size: 0.75em; background: {}; color: {};", 
                                if filter.get() == Filter::Healthy { "#10b981" } else { "transparent" },
                                if filter.get() == Filter::Healthy { "white" } else { "#64748b" })> "HEALTHY" </button>
                    </div>
                </div>

                <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: 15px;">
                    <Transition fallback=|| view! { <p>"Syncing Environments..."</p> }>
                        {move || health_data.get().unwrap_or_default().into_iter()
                            .filter(|(_, cust)| {
                                let has_fail = cust.groups.values().any(|g| !g.is_healthy);
                                match filter.get() {
                                    Filter::All => true,
                                    Filter::Failed => has_fail,
                                    Filter::Healthy => !has_fail,
                                }
                            })
                            .map(|(_, cust)| {
                                let is_critical = cust.groups.values().any(|g| !g.is_healthy);
                                let status_color = if is_critical { "#ef4444" } else { "#10b981" };

                                view! {
                                    <div style=format!("background: white; border-radius: 12px; padding: 18px; box-shadow: 0 4px 6px -1px rgba(0,0,0,0.05); border-top: 6px solid {};", status_color)>
                                        <h3 style="margin: 0 0 15px 0; color: #1e293b; font-size: 1.1em; font-weight: 800;">{cust.name}</h3>
                                        <div style="display: flex; flex-wrap: wrap; gap: 8px;">
                                            {cust.groups.values().cloned().map(|g| {
                                                let bg = if g.is_healthy { "#f0fdf4" } else { "#fee2e2" };
                                                let fg = if g.is_healthy { "#166534" } else { "#991b1b" };
                                                let dot = if g.is_healthy { "#22c55e" } else { "#ef4444" };
                                                
                                                view! {
                                                    <div style=format!("background: {}; color: {}; padding: 4px 10px; border-radius: 6px; font-weight: 800; font-size: 0.72em; border: 1px solid rgba(0,0,0,0.05); display: flex; align-items: center; gap: 8px;", bg, fg)>
                                                        <div style=format!("width: 8px; height: 8px; border-radius: 50%; background: {};", dot)></div>
                                                        {format!("{}: {}", g.name, g.total)}
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>
                                }
                            }).collect_view()}
                    </Transition>
                </div>
            </div>
        </div>
    }
}

fn main() { mount_to_body(|| view! { <App /> }) }
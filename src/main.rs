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
    pub ok: usize,
    pub err: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CustomerSummary {
    pub name: String,
    pub groups: BTreeMap<String, GroupStatus>,
}

async fn fetch_health_data() -> BTreeMap<String, CustomerSummary> {
    let mut customers: BTreeMap<String, CustomerSummary> = BTreeMap::new();
    let mut winning_files: BTreeMap<String, String> = BTreeMap::new();
    let base_url = "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/";

    if let Ok(resp) = Request::get(base_url).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(objects) = json.get("objects").and_then(|o| o.as_array()) {
                for obj in objects {
                    if let Some(name) = obj.get("name").and_then(|n| n.as_str()) {
                        // Filter out non-relevant files and generic data.json
                        if name.ends_with("_latest.json") && name != "data.json" {
                            let parts: Vec<&str> = name.split('_').collect();
                            if parts.len() >= 2 {
                                let key = format!("{}_{}", parts[0], parts[1].to_uppercase());
                                winning_files.insert(key, name.to_string());
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

                            let mut ok_count = 0;
                            let mut err_count = 0;
                            for inst in &instances {
                                let s = inst.instance_status.as_deref().unwrap_or("").to_uppercase();
                                let h = inst.health_status.as_deref().unwrap_or("").to_lowercase();
                                if s == "RUNNING" && h == "passed" {
                                    ok_count += 1;
                                } else {
                                    err_count += 1;
                                }
                            }

                            cust.groups.insert(grp_name.clone(), GroupStatus {
                                name: grp_name,
                                total: instances.len(),
                                ok: ok_count,
                                err: err_count,
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
    let (refresh, set_refresh) = create_signal(0);
    let health_data = create_resource(move || refresh.get(), |_| async move { fetch_health_data().await });
    
    core::mem::forget(Interval::new(60_000, move || set_refresh.update(|n| *n += 1)));

    view! {
        <div style="padding: 30px; background: #f1f5f9; min-height: 100vh; font-family: sans-serif;">
            <div style="max-width: 1100px; margin: auto;">
                <h2 style="color: #0f172a; font-weight: 900; margin-bottom: 30px;">"JDE GLOBAL HEALTH DASHBOARD"</h2>

                // --- Summary Chart Section ---
                <Transition fallback=|| view! { <p>"Calculating Summary..."</p> }>
                    {move || {
                        let data = health_data.get().unwrap_or_default();
                        let total_ok: usize = data.values().flat_map(|c| c.groups.values().map(|g| g.ok)).sum();
                        let total_err: usize = data.values().flat_map(|c| c.groups.values().map(|g| g.err)).sum();
                        let total = total_ok + total_err;
                        let ok_pct = if total > 0 { (total_ok as f32 / total as f32) * 100.0 } else { 0.0 };

                        view! {
                            <div style="background: white; border-radius: 12px; padding: 20px; margin-bottom: 30px; box-shadow: 0 4px 6px -1px rgba(0,0,0,0.05);">
                                <div style="display: flex; justify-content: space-between; margin-bottom: 10px; font-weight: 800; font-size: 0.9em;">
                                    <span>"SYSTEM WIDE HEALTH"</span>
                                    <span style=format!("color: {};", if ok_pct < 100.0 { "#ef4444" } else { "#10b981" })>{format!("{:.1}%", ok_pct)}</span>
                                </div>
                                <div style="width: 100%; height: 12px; background: #fee2e2; border-radius: 6px; overflow: hidden; display: flex;">
                                    <div style=format!("width: {}%; background: #10b981; height: 100%; transition: 0.5s;", ok_pct)></div>
                                </div>
                                <div style="display: flex; gap: 20px; margin-top: 15px; font-size: 0.8em; font-weight: 700; color: #64748b;">
                                    <span>{format!("{} INSTANCES OK", total_ok)}</span>
                                    <span>{format!("{} INSTANCES FAILED", total_err)}</span>
                                </div>
                            </div>
                        }
                    }}
                </Transition>

                // --- Customer Cards Section ---
                <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(340px, 1fr)); gap: 20px;">
                    <Transition fallback=|| view! { <p>"Syncing Logs..."</p> }>
                        {move || health_data.get().unwrap_or_default().into_iter().map(|(_, cust)| {
                            let has_error = cust.groups.values().any(|g| g.err > 0);
                            let border_color = if has_error { "#ef4444" } else { "#10b981" };

                            view! {
                                <div style=format!("background: white; border-radius: 12px; padding: 24px; border-left: 6px solid {}; box-shadow: 0 4px 6px -1px rgba(0,0,0,0.05);", border_color)>
                                    <h3 style="margin: 0 0 20px 0; color: #1e293b; font-size: 1.3em; font-weight: 900;">{cust.name}</h3>
                                    
                                    <div style="display: flex; flex-wrap: wrap; gap: 10px; margin-bottom: 20px;">
                                        {cust.groups.values().cloned().map(|g| {
                                            let is_err = g.err > 0;
                                            let bg = if is_err { "#fee2e2" } else { "#f0fdf4" };
                                            let dot = if is_err { "#ef4444" } else { "#22c55e" };
                                            let text = if is_err { "#991b1b" } else { "#166534" };
                                            
                                            view! {
                                                <div style=format!("background: {}; color: {}; padding: 6px 12px; border-radius: 8px; font-weight: 800; font-size: 0.75em; display: flex; align-items: center; gap: 8px;", bg, text)>
                                                    <div style=format!("width: 8px; height: 8px; border-radius: 50%; background: {};", dot)></div>
                                                    {format!("{}: {}", g.name, g.total)}
                                                </div>
                                            }
                                        }).collect_view()}
                                    </div>

                                    <div style="border-top: 1px solid #f1f5f9; padding-top: 15px; display: flex; justify-content: space-between; font-size: 0.75em; font-weight: 700; color: #94a3b8;">
                                        <span>{format!("{} TOTAL CHECKED", cust.groups.values().map(|g| g.total).sum::<usize>())}</span>
                                        <span style=format!("color: {};", border_color)>
                                            {if has_error { "ATTENTION REQUIRED" } else { "SYSTEMS NOMINAL" }}
                                        </span>
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
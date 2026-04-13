use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::collections::BTreeMap;
use gloo_timers::callback::Interval;

// --- Data Models ---

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

// --- Helper Functions ---

// Extracts CUSTOMER and GROUP from filename patterns like:
// "CUSTOMERNAME_SERVERGROUP_latest.json"
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
    
    // Using your specific Ashburn Region Bucket PAR URL
    let base_url = "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/";

    // 1. Fetch the list of objects in the bucket
    if let Ok(resp) = Request::get(base_url).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(objects) = json.get("objects").and_then(|o| o.as_array()) {
                for obj in objects {
                    if let Some(name) = obj.get("name").and_then(|n| n.as_str()) {
                        // 2. Only process the "latest" status files
                        if name.ends_with("_latest.json") {
                            if let Some((cust_name, grp_name)) = parse_meta(name) {
                                let file_url = format!("{}{}", base_url, name);
                                
                                // 3. Fetch the content of each specific JSON file
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
                                            if status == "RUNNING" || health == "Passed" {
                                                ok += 1;
                                            } else {
                                                err += 1;
                                            }
                                        }

                                        cust.groups.insert(grp_name.clone(), GroupStatus {
                                            name: grp_name,
                                            total: instances.len(),
                                            ok,
                                            err,
                                        });
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
    
    // Resource that fetches data whenever refresh_count changes
    let health_data = create_resource(move || refresh_count.get(), |_| async move {
        fetch_all_health_data().await
    });

    // Setup an interval to auto-refresh the UI every 60 seconds
    core::mem::forget(Interval::new(60_000, move || {
        set_refresh_count.update(|n| *n += 1);
    }));

    view! {
        <div style="padding: 40px; background: #f8fafc; min-height: 100vh; font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;">
            <div style="max-width: 1400px; margin: auto;">
                <h1 style="color: #1e293b; margin-bottom: 30px; font-weight: 800; letter-spacing: -1px;">
                    "JDE Global Health Dashboard"
                </h1>
                
                <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(420px, 1fr)); gap: 25px;">
                    <Transition fallback=|| view! { <p style="color: #64748b;">"Syncing with Oracle Cloud Infrastructure..."</p> }>
                        {move || {
                            health_data.get().unwrap_or_default().into_iter().map(|(_, cust)| {
                                let total_ok: usize = cust.groups.values().map(|g| g.ok).sum();
                                let total_err: usize = cust.groups.values().map(|g| g.err).sum();
                                let total_inst: usize = cust.groups.values().map(|g| g.total).sum();
                                
                                // Card color: Red if any instance is down, Green if all perfect
                                let status_color = if total_inst == 0 { "#94a3b8" } 
                                                 else if total_err > 0 { "#ef4444" } 
                                                 else { "#22c55e" };

                                let health_pct = if total_inst > 0 { 
                                    (total_ok as f32 / total_inst as f32 * 100.0) as i32 
                                } else { 0 };

                                view! {
                                    <div style=format!("border-left: 6px solid {}; background: white; padding: 25px; border-radius: 12px; box-shadow: 0 4px 6px -1px rgba(0,0,0,0.1);", status_color)>
                                        <h2 style="margin: 0 0 15px 0; color: #1e293b; text-transform: uppercase; font-size: 1.4em;">{cust.name}</h2>
                                        
                                        <div style="display: flex; flex-wrap: wrap; gap: 8px; margin-bottom: 25px;">
                                            {cust.groups.values().cloned().map(|g| {
                                                view! {
                                                    <span style="background: #f1f5f9; color: #475569; padding: 4px 12px; border-radius: 15px; font-weight: 700; font-size: 0.75em; border: 1px solid #e2e8f0;">
                                                        {format!("{}: {}", g.name, g.total)}
                                                    </span>
                                                }
                                            }).collect_view()}
                                        </div>

                                        <div style="border-top: 1px solid #f1f5f9; padding-top: 20px; display: flex; align-items: center; justify-content: space-between;">
                                            <div style="font-size: 0.85em; font-weight: 600; display: flex; gap: 12px;">
                                                <span style="color: #22c55e;">{format!("● {} OK", total_ok)}</span>
                                                <span style="color: #ef4444;">{format!("● {} ERR", total_err)}</span>
                                                <span style="color: #94a3b8;">"● 0 UNK"</span>
                                            </div>
                                            
                                            <div style=format!("width: 60px; height: 60px; border-radius: 50%; border: 5px solid #f1f5f9; border-top-color: {}; display: flex; align-items: center; justify-content: center; font-size: 0.9em; font-weight: 800; color: #1e293b;", status_color)>
                                                {format!("{}%", health_pct)}
                                            </div>
                                        </div>
                                        
                                        <button style="margin-top: 20px; width: 100%; padding: 12px; border: none; background: #1e293b; color: white; border-radius: 8px; font-weight: bold; cursor: pointer; font-size: 0.9em;"
                                                on:click=|_| { /* Future drill-down logic */ }>
                                            "VIEW DETAILS"
                                        </button>
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

fn main() { 
    mount_to_body(|| view! { <App /> }) 
}
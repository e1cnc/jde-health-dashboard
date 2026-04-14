use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::collections::BTreeMap;

// --- Data Models ---

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    // These match the snake_case keys in your JSON files (e.g., LSJJOLDTR_ps_latest.json)
    pub instance_status: Option<String>,
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

// --- Data Fetching Logic ---

async fn fetch_jde_data() -> Result<BTreeMap<String, CustomerSummary>, String> {
    let mut customers: BTreeMap<String, CustomerSummary> = BTreeMap::new();
    
    // Using the Ashburn region endpoint as confirmed in your OCI configuration
    let targets = vec![
       //"LSJJNEWTR", "DV", "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuz/id7bn4roxxyb/b/JDE_Monitoring_Data/o/LSJJNEWTR_dv_latest.json"),
        //SJJNEWTR", "PY", "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuz/id7bn4roxxyb/b/JDE_Monitoring_Data/o/LSJJNEWTR_py_latest.json"),
        //LSJJOLDTR", "PS", "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuz/id7bn4roxxyb/b/JDE_Monitoring_Data/o/LSJJOLDTR_ps_latest.json"),
        //LSJJOLDTR", "PY", "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuz/id7bn4roxxyb/b/JDE_Monitoring_Data/o/LSJJOLDTR_py_latest.json"),
        ("LSJJNEWTR", "DV", "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/LSJJNEWTR_dv_latest.json"),
        ("LSJJNEWTR", "PY", "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/LSJJNEWTR_py_latest.json"),
        ("LSJJOLDTR", "PS", "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/LSJJOLDTR_ps_latest.json"),
        ("LSJJOLDTR", "PY", "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/LSJJOLDTR_py_latest.json"),
    ];

    for (c_name, g_name, url) in targets {
        if let Ok(resp) = Request::get(url).send().await {
            if let Ok(instances) = resp.json::<Vec<HealthInstance>>().await {
                let cust = customers.entry(c_name.to_string()).or_insert(CustomerSummary {
                    name: c_name.to_string(),
                    groups: BTreeMap::new(),
                });

                let (mut ok_count, mut err_count) = (0, 0);
                for inst in &instances {
                    let status = inst.instance_status.as_deref().unwrap_or("").to_uppercase();
                    let health = inst.health_status.as_deref().unwrap_or("").to_lowercase();
                    
                    if status == "RUNNING" && health == "passed" {
                        ok_count += 1;
                    } else {
                        err_count += 1;
                    }
                }

                cust.groups.insert(g_name.to_string(), GroupStatus {
                    name: g_name.to_string(),
                    total: instances.len(),
                    ok: ok_count,
                    err: err_count,
                });
            }
        }
    }
    
    if customers.is_empty() {
        return Err("No data retrieved from OCI storage".to_string());
    }
    Ok(customers)
}

// --- UI Components ---

#[component]
fn App() -> impl IntoView {
    let health_resource = create_resource(|| (), |_| async move { fetch_jde_data().await });

    view! {
        <div style="padding: 40px; background: #f8fafc; min-height: 100vh; font-family: system-ui, sans-serif;">
            <div style="max-width: 1100px; margin: auto;">
                <h1 style="color: #0f172a; font-size: 2.2em; font-weight: 900; margin-bottom: 40px; letter-spacing: -0.02em;">
                    "JDE GLOBAL HEALTH DASHBOARD"
                </h1>

                <Transition fallback=|| view! { <p>"Loading environment statuses..."</p> }>
                    {move || match health_resource.get() {
                        None => view! { <p>"Connecting..."</p> }.into_view(),
                        Some(Err(e)) => view! { <p style="color: #ef4444; font-weight: bold;">{format!("Error: {}", e)}</p> }.into_view(),
                        Some(Ok(data)) => {
                            let total_ok: usize = data.values().flat_map(|c| c.groups.values().map(|g| g.ok)).sum();
                            let total_err: usize = data.values().flat_map(|c| c.groups.values().map(|g| g.err)).sum();
                            let total = total_ok + total_err;
                            let ok_pct = if total > 0 { (total_ok as f32 / total as f32) * 100.0 } else { 0.0 };

                            view! {
                                // System-Wide Summary
                                <div style="background: white; border-radius: 16px; padding: 30px; margin-bottom: 40px; box-shadow: 0 4px 6px -1px rgba(0,0,0,0.1);">
                                    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px;">
                                        <span style="font-weight: 800; color: #1e293b; text-transform: uppercase; font-size: 0.9em; letter-spacing: 0.05em;">"System Wide Health"</span>
                                        <span style=move || format!("color: {}; font-weight: 900; font-size: 1.2em;", if ok_pct < 100.0 { "#ef4444" } else { "#10b981" })>
                                            {format!("{:.1}%", ok_pct)}
                                        </span>
                                    </div>
                                    <div style="width: 100%; height: 12px; background: #f1f5f9; border-radius: 6px; overflow: hidden;">
                                        <div style=move || format!("width: {}%; background: #10b981; height: 100%; transition: width 1s ease-in-out;", ok_pct)></div>
                                    </div>
                                    <div style="margin-top: 15px; font-size: 0.85em; font-weight: 700; color: #64748b; display: flex; gap: 20px;">
                                        <span>{format!("{} INSTANCES OK", total_ok)}</span>
                                        <span>{format!("{} INSTANCES FAILED", total_err)}</span>
                                    </div>
                                </div>

                                // Customer Grid
                                <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(340px, 1fr)); gap: 25px;">
                                    {data.into_iter().map(|(_, cust)| {
                                        view! {
                                            <div style="background: white; border-radius: 16px; padding: 25px; border-left: 5px solid #ef4444; box-shadow: 0 2px 4px rgba(0,0,0,0.05);">
                                                <h2 style="margin: 0 0 20px 0; color: #0f172a; font-size: 1.6em; font-weight: 800;">{cust.name}</h2>
                                                <div style="display: flex; flex-wrap: wrap; gap: 10px;">
                                                    {cust.groups.values().cloned().map(|g| {
                                                        let has_error = g.err > 0;
                                                        let bg = if has_error { "#fee2e2" } else { "#f0fdf4" };
                                                        let text = if has_error { "#991b1b" } else { "#166534" };
                                                        let dot = if has_error { "#ef4444" } else { "#22c55e" };
                                                        
                                                        view! {
                                                            <div style=format!("background: {}; color: {}; padding: 8px 14px; border-radius: 10px; font-weight: 800; font-size: 0.8em; display: flex; align-items: center; gap: 8px;", bg, text)>
                                                                <div style=format!("width: 8px; height: 8px; border-radius: 50%; background: {};", dot)></div>
                                                                {format!("{}: {}/{}", g.name, g.ok, g.total)}
                                                            </div>
                                                        }
                                                    }).collect_view()}
                                                </div>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            }.into_view()
                        }
                    }}
                </Transition>
            </div>
        </div>
    }
}

fn main() {
    mount_to_body(|| view! { <App /> })
}
use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::collections::BTreeMap;

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

async fn fetch_single_target_data() -> BTreeMap<String, CustomerSummary> {
    let mut customers: BTreeMap<String, CustomerSummary> = BTreeMap::new();
    // TARGET: LSJJOLDTR - PS Group
    let test_url = "/https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/LSJJOLDTR_ps_latest.json";

    if let Ok(file_resp) = Request::get(test_url).send().await {
        if let Ok(instances) = file_resp.json::<Vec<HealthInstance>>().await {
            let cust_name = "LSJJOLDTR".to_string();
            let grp_name = "PS".to_string();

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
    customers
}

#[component]
fn App() -> impl IntoView {
    let health_data = create_resource(|| (), |_| async move { fetch_single_target_data().await });

    view! {
        <div style="padding: 30px; background: #f1f5f9; min-height: 100vh; font-family: sans-serif;">
            <div style="max-width: 900px; margin: auto;">
                <h2 style="color: #0f172a; font-weight: 900; margin-bottom: 30px;">"JDE SINGLE TARGET TEST"</h2>

                <Transition fallback=|| view! { <p>"Fetching Test Data..."</p> }>
                    {move || {
                        let data = health_data.get().unwrap_or_default();
                        let total_ok: usize = data.values().flat_map(|c| c.groups.values().map(|g| g.ok)).sum();
                        let total_err: usize = data.values().flat_map(|c| c.groups.values().map(|g| g.err)).sum();
                        let total = total_ok + total_err;
                        let ok_pct = if total > 0 { (total_ok as f32 / total as f32) * 100.0 } else { 0.0 };

                        view! {
                            // --- Summary Chart ---
                            <div style="background: white; border-radius: 12px; padding: 25px; margin-bottom: 30px; box-shadow: 0 4px 10px rgba(0,0,0,0.05);">
                                <div style="display: flex; justify-content: space-between; margin-bottom: 15px; font-weight: 900; font-size: 0.9em;">
                                    <span>"LSJJOLDTR: PS HEALTH"</span>
                                    <span style=move || format!("color: {};", if ok_pct < 100.0 { "#ef4444" } else { "#10b981" })>
                                        {format!("{:.1}%", ok_pct)}
                                    </span>
                                </div>
                                <div style="width: 100%; height: 16px; background: #fee2e2; border-radius: 8px; overflow: hidden; display: flex;">
                                    <div style=move || format!("width: {}%; background: #10b981; height: 100%; transition: 0.8s ease-in-out;", ok_pct)></div>
                                </div>
                                <div style="display: flex; gap: 25px; margin-top: 20px; font-size: 0.8em; font-weight: 800; color: #64748b;">
                                    <span style="color: #10b981;">{format!("OK: {}", total_ok)}</span>
                                    <span style="color: #ef4444;">{format!("FAILED: {}", total_err)}</span>
                                </div>
                            </div>

                            // --- Customer Card ---
                            {data.into_iter().map(|(_, cust)| {
                                view! {
                                    <div style="background: white; border-radius: 12px; padding: 24px; box-shadow: 0 4px 6px rgba(0,0,0,0.05);">
                                        <h3 style="margin: 0 0 20px 0; color: #1e293b; font-size: 1.4em; font-weight: 900;">{cust.name}</h3>
                                        <div style="display: flex; gap: 12px;">
                                            {cust.groups.values().cloned().map(|g| {
                                                let is_err = g.err > 0;
                                                let bg = if is_err { "#fee2e2" } else { "#f0fdf4" };
                                                let dot = if is_err { "#ef4444" } else { "#22c55e" };
                                                let text = if is_err { "#991b1b" } else { "#166534" };
                                                view! {
                                                    <div style=format!("background: {}; color: {}; padding: 8px 16px; border-radius: 8px; font-weight: 800; font-size: 0.8em; display: flex; align-items: center; gap: 10px;", bg, text)>
                                                        <div style=format!("width: 10px; height: 10px; border-radius: 50%; background: {};", dot)></div>
                                                        {format!("{}: {}/{}", g.name, g.ok, g.total)}
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>
                                }
                            }).collect_view()}
                        }
                    }}
                </Transition>
            </div>
        </div>
    }
}

fn main() { mount_to_body(|| view! { <App /> }) }
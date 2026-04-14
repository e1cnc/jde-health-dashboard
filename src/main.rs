use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    pub instance_status: Option<String>,
    pub health_status: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EnvStatus {
    pub customer: String,
    pub env_name: String,
    pub total: usize,
    pub ok: usize,
    pub err: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum Filter { All, Failed, Healthy }

async fn fetch_jde_health_data() -> Result<Vec<EnvStatus>, String> {
    let mut results = Vec::new();
    
    // Updated targets with correct uppercase 'PY' for OCI Object Storage case-sensitivity
    let targets = vec![
         ("LSJJNEWTR", "DV", "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/LSJJNEWTR_dv_latest.json"),
        ("LSJJNEWTR", "PY", "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/LSJJNEWTR_py_latest.json"),
        ("LSJJOLDTR", "PS", "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/LSJJOLDTR_ps_latest.json"),
        ("LSJJOLDTR", "PY", "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/LSJJOLDTR_PY_latest.json"),
    ];

    for (cust, env, url) in targets {
        if let Ok(resp) = Request::get(url).send().await {
            if let Ok(instances) = resp.json::<Vec<HealthInstance>>().await {
                let mut ok = 0;
                let mut err = 0;

                for inst in &instances {
                    let s = inst.instance_status.as_deref().unwrap_or("").to_uppercase();
                    let h = inst.health_status.as_deref().unwrap_or("").to_lowercase();
                    // Status logic: RUNNING and passed
                    if s == "RUNNING" && h == "passed" { ok += 1; } else { err += 1; }
                }

                results.push(EnvStatus {
                    customer: cust.to_string(),
                    env_name: env.to_string(),
                    total: instances.len(),
                    ok,
                    err,
                });
            }
        }
    }
    
    if results.is_empty() { return Err("No data retrieved. Verify CORS and OCI paths.".to_string()); }
    Ok(results)
}

#[component]
fn App() -> impl IntoView {
    let (filter, set_filter) = create_signal(Filter::All);
    let health_data = create_resource(|| (), |_| async move { fetch_jde_health_data().await });

    view! {
        <div style="padding: 20px; background: #f8fafc; min-height: 100vh; font-family: 'Inter', system-ui, sans-serif;">
            <div style="max-width: 1100px; margin: auto;">
                
                // Header & Controls
                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 30px; border-bottom: 1px solid #e2e8f0; padding-bottom: 15px;">
                    <h2 style="margin: 0; color: #0f172a; font-weight: 800; letter-spacing: -0.025em;">"JDE HEALTH DASHBOARD"</h2>
                    
                    <div style="display: flex; gap: 4px; background: #f1f5f9; padding: 4px; border-radius: 10px; border: 1px solid #e2e8f0;">
                        {move || vec![Filter::All, Filter::Failed, Filter::Healthy].into_iter().map(|f| {
                            let label = match f { Filter::All => "ALL", Filter::Failed => "FAILED", Filter::Healthy => "HEALTHY" };
                            let is_active = filter.get() == f;
                            view! {
                                <button on:click=move |_| set_filter.set(f)
                                    style=format!("border: none; padding: 8px 16px; border-radius: 7px; cursor: pointer; font-weight: 700; font-size: 0.75rem; transition: all 0.2s; background: {}; color: {}; shadow: {};", 
                                        if is_active { "#1e293b" } else { "transparent" },
                                        if is_active { "white" } else { "#64748b" },
                                        if is_active { "0 1px 3px rgba(0,0,0,0.1)" } else { "none" })>
                                    {label}
                                </button>
                            }
                        }).collect_view()}
                    </div>
                </div>

                <Transition fallback=|| view! { <div style="color: #64748b;">"Requesting data from OCI US-Ashburn..."</div> }>
                    {move || health_data.get().map(|res| match res {
                        Err(e) => view! { <div style="background: #fef2f2; color: #991b1b; padding: 15px; border-radius: 8px; border: 1px solid #fecaca;">{e}</div> }.into_view(),
                        Ok(items) => {
                            let filtered: Vec<_> = items.into_iter().filter(|item| {
                                match filter.get() {
                                    Filter::All => true,
                                    Filter::Failed => item.err > 0,
                                    Filter::Healthy => item.err == 0,
                                }
                            }).collect();

                            view! {
                                <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 16px;">
                                    {filtered.into_iter().map(|item| {
                                        let is_healthy = item.err == 0;
                                        view! {
                                            <div style=format!("background: white; border-radius: 12px; padding: 18px; box-shadow: 0 4px 6px -1px rgba(0,0,0,0.1); border-top: 5px solid {};", if is_healthy { "#10b981" } else { "#ef4444" })>
                                                <div style="font-size: 0.7rem; font-weight: 700; color: #94a3b8; text-transform: uppercase;">{item.customer}</div>
                                                <div style="font-size: 1.25rem; font-weight: 900; color: #1e293b; margin-top: 2px; margin-bottom: 12px;">{item.env_name}</div>
                                                
                                                <div style="display: flex; justify-content: space-between; align-items: flex-end; padding-top: 10px; border-top: 1px solid #f1f5f9;">
                                                    <div>
                                                        <div style=format!("font-size: 0.8rem; font-weight: 800; color: {};", if is_healthy { "#059669" } else { "#dc2626" })>
                                                            {if is_healthy { "HEALTHY" } else { "ERROR" }}
                                                        </div>
                                                        <div style="font-size: 0.75rem; color: #64748b;">{format!("{}/{} OK", item.ok, item.total)}</div>
                                                    </div>
                                                    <div style=format!("font-size: 1.25rem; font-weight: 900; color: {};", if is_healthy { "#10b981" } else { "#ef4444" })>
                                                        {format!("{:.0}%", (item.ok as f32 / item.total as f32) * 100.0)}
                                                    </div>
                                                </div>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            }.into_view()
                        }
                    })}
                </Transition>
            </div>
        </div>
    }
}

fn main() { mount_to_body(|| view! { <App /> }) }
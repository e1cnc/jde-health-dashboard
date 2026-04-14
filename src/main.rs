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
    pub server_group: String,
    pub total: usize,
    pub ok: usize,
    pub err: usize,
}

// Ensure fields match the OCI JSON structure exactly
#[derive(Deserialize, Debug)]
pub struct OCIObject { 
    pub name: String 
}

#[derive(Deserialize, Debug)]
pub struct OCIListResponse { 
    pub objects: Vec<OCIObject> 
}

#[derive(Clone, Copy, PartialEq)]
enum Filter { All, Failed, Healthy }

const LIST_API_URL: &str = "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/";

async fn fetch_dynamic_jde_health() -> Result<Vec<EnvStatus>, String> {
    let mut results = Vec::new();

    // 1. Fetch Object List with query parameters to force JSON
    let resp = Request::get(&format!("{}?format=json", LIST_API_URL))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Network failure: {}", e))?;

    if !resp.ok() {
        return Err(format!("OCI Server Error: Status {}", resp.status()));
    }

    // Capture raw text for debugging and parsing
    let body_text = resp.text().await.map_err(|_| "Could not read response body")?;
    
    // Attempt to parse the JSON list
    let list_data: OCIListResponse = serde_json::from_str(&body_text)
        .map_err(|e| {
            // Log to F12 Console for troubleshooting
            web_sys::console::log_1(&format!("JSON Parse Error: {}. Body: {}", e, body_text).into());
            format!("Mapping Error: {}. Check browser console for raw API output.", e)
        })?;

    // 2. Filter for files ending in _latest.json
    let target_files: Vec<String> = list_data.objects.into_iter()
        .map(|obj| obj.name)
        .filter(|name| name.ends_with("_latest.json"))
        .collect();

    for filename in target_files {
        let file_url = format!("https://objectstorage.us-ashburn-1.oraclecloud.com/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/{}", filename);
        
        if let Ok(file_resp) = Request::get(&file_url).send().await {
            if let Ok(instances) = file_resp.json::<Vec<HealthInstance>>().await {
                let parts: Vec<&str> = filename.split('_').collect();
                let customer = parts.get(0).unwrap_or(&"UNKNOWN").to_string();
                let group = parts.get(1).unwrap_or(&"UNKNOWN").to_uppercase();

                let (mut ok, mut err) = (0, 0);
                for inst in &instances {
                    let s = inst.instance_status.as_deref().unwrap_or("").to_uppercase();
                    let h = inst.health_status.as_deref().unwrap_or("").to_lowercase();
                    if s == "RUNNING" && h == "passed" { ok += 1; } else { err += 1; }
                }

                results.push(EnvStatus {
                    customer,
                    server_group: group,
                    total: instances.len(),
                    ok,
                    err,
                });
            }
        }
    }

    if results.is_empty() { return Err("Dashboard online, but no health files found in bucket.".to_string()); }
    
    results.sort_by(|a, b| a.customer.cmp(&b.customer));
    Ok(results)
}

#[component]
fn App() -> impl IntoView {
    let (filter, set_filter) = create_signal(Filter::All);
    let health_data = create_resource(|| (), |_| async move { fetch_dynamic_jde_health().await });

    view! {
        <div style="padding: 20px; background: #f8fafc; min-height: 100vh; font-family: sans-serif;">
            <div style="max-width: 1200px; margin: auto;">
                
                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 30px;">
                    <div>
                        <h2 style="margin: 0; color: #0f172a; font-weight: 800; font-size: 1.8rem;">"JDE GLOBAL MONITOR"</h2>
                        <p style="margin: 5px 0 0 0; font-size: 0.75rem; color: #64748b; font-weight: 700;">"AUTO-DISCOVERY ACTIVE"</p>
                    </div>
                    
                    <div style="display: flex; gap: 4px; background: #f1f5f9; padding: 4px; border-radius: 10px; border: 1px solid #e2e8f0;">
                        <button on:click=move |_| set_filter.set(Filter::All)
                            style=move || format!("border: none; padding: 8px 16px; border-radius: 7px; cursor: pointer; font-weight: 700; background: {}; color: {};", if filter.get() == Filter::All { "#1e293b" } else { "transparent" }, if filter.get() == Filter::All { "white" } else { "#64748b" })>"ALL"</button>
                        <button on:click=move |_| set_filter.set(Filter::Failed)
                            style=move || format!("border: none; padding: 8px 16px; border-radius: 7px; cursor: pointer; font-weight: 700; background: {}; color: {};", if filter.get() == Filter::Failed { "#ef4444" } else { "transparent" }, if filter.get() == Filter::Failed { "white" } else { "#64748b" })>"FAILED"</button>
                        <button on:click=move |_| set_filter.set(Filter::Healthy)
                            style=move || format!("border: none; padding: 8px 16px; border-radius: 7px; cursor: pointer; font-weight: 700; background: {}; color: {};", if filter.get() == Filter::Healthy { "#10b981" } else { "transparent" }, if filter.get() == Filter::Healthy { "white" } else { "#64748b" })>"HEALTHY"</button>
                    </div>
                </div>

                <Transition fallback=|| view! { <p style="color: #64748b; font-weight: 700;">"Scanning OCI Storage..."</p> }>
                    {move || health_data.get().map(|res| match res {
                        Err(e) => view! { 
                            <div style="background: #fee2e2; border: 1px solid #ef4444; color: #b91c1c; padding: 20px; border-radius: 12px; font-weight: 600;">
                                {e}
                                <p style="font-size: 0.8rem; margin-top: 10px; color: #7f1d1d;">"Troubleshooting: If console shows XML, check Bucket Visibility and Listing settings."</p>
                            </div> 
                        }.into_view(),
                        Ok(items) => {
                            let filtered: Vec<_> = items.into_iter().filter(|item| {
                                match filter.get() {
                                    Filter::All => true,
                                    Filter::Failed => item.err > 0,
                                    Filter::Healthy => item.err == 0,
                                }
                            }).collect();

                            view! {
                                <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 20px;">
                                    {filtered.into_iter().map(|item| {
                                        let is_healthy = item.err == 0;
                                        view! {
                                            <div style=format!("background: white; border-radius: 12px; padding: 20px; box-shadow: 0 4px 6px -1px rgba(0,0,0,0.1); border-top: 6px solid {};", if is_healthy { "#10b981" } else { "#ef4444" })>
                                                <div style="font-size: 0.7rem; font-weight: 800; color: #94a3b8; text-transform: uppercase;">{item.customer}</div>
                                                <div style="font-size: 1.4rem; font-weight: 900; color: #1e293b; margin-bottom: 15px;">{item.server_group}</div>
                                                <div style="display: flex; justify-content: space-between; align-items: flex-end; border-top: 1px solid #f1f5f9; padding-top: 12px;">
                                                    <div>
                                                        <div style=format!("font-size: 0.85rem; font-weight: 800; color: {};", if is_healthy { "#059669" } else { "#dc2626" })>
                                                            {if is_healthy { "HEALTHY" } else { "ACTION REQUIRED" }}
                                                        </div>
                                                        <div style="font-size: 0.8rem; color: #64748b;">{format!("{}/{} OK", item.ok, item.total)}</div>
                                                    </div>
                                                    <div style=format!("font-size: 1.6rem; font-weight: 900; color: {};", if is_healthy { "#10b981" } else { "#ef4444" })>
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

fn main() {
    mount_to_body(|| view! { <App /> })
}
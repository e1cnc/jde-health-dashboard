use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    #[serde(default)]
    pub customer_name: String,

    #[serde(default)]
    pub host_name: String,

    // Maps "group_name" from your binary output to the Group column
    #[serde(default, alias = "group_name", alias = "ServerGroup")]
    pub instance_name: String,

    #[serde(default)]
    pub status: String,

    // Maps "timestamp" or "lastSync" to the Last Update column
    #[serde(default, alias = "timestamp", alias = "lastSync")]
    pub last_sync: String,
}

#[component]
fn App() -> impl IntoView {
    let health_data = create_resource(|| (), |_| async move { fetch_health_data().await });

    view! {
        <div style="padding: 20px; font-family: sans-serif; max-width: 1200px; margin: auto;">
            <h2 style="color: #004488; border-bottom: 2px solid #004488; padding-bottom: 10px; font-weight: bold;">
                "JDE Global Health Monitor Dashboard"
            </h2>
            
            <table style="width: 100%; border-collapse: collapse; margin-top: 20px; box-shadow: 0 4px 12px rgba(0,0,0,0.1); border-radius: 8px; overflow: hidden;">
                <thead>
                    <tr style="background-color: #004488; color: white; text-align: left;">
                        <th style="padding: 15px; border: 1px solid #ddd;">"Customer"</th>
                        <th style="padding: 15px; border: 1px solid #ddd;">"Host"</th>
                        <th style="padding: 15px; border: 1px solid #ddd;">"Instance"</th>
                        <th style="padding: 15px; border: 1px solid #ddd;">"Status"</th>
                        <th style="padding: 15px; border: 1px solid #ddd;">"Last Update"</th>
                    </tr>
                </thead>
                <tbody>
                    <Transition fallback=move || view! { <tr><td colspan="5" style="text-align: center; padding: 40px; color: #666;">"Fetching latest health data..."</td></tr> }>
                        {move || health_data.get().map(|data| {
                            match data {
                                Ok(instances) => {
                                    if instances.is_empty() {
                                        return view! { <tr><td colspan="5" style="text-align: center; padding: 20px;">"No health records found."</td></tr> }.into_view();
                                    }
                                    instances.into_iter().map(|inst| {
                                        let (bg, txt) = if inst.status == "RUNNING" { ("#d4edda", "#155724") } else { ("#f8d7da", "#721c24") };
                                        view! {
                                            <tr style="border-bottom: 1px solid #eee;">
                                                <td style="padding: 12px; border: 1px solid #ddd;">{inst.customer_name}</td>
                                                <td style="padding: 12px; border: 1px solid #ddd;">{inst.host_name}</td>
                                                <td style="padding: 12px; border: 1px solid #ddd;">{inst.instance_name}</td>
                                                <td style="padding: 12px; border: 1px solid #ddd; text-align: center;">
                                                    <span style="padding: 6px 12px; border-radius: 20px; background-color: {bg}; color: {txt}; font-weight: bold; font-size: 0.9em;">
                                                        {inst.status}
                                                    </span>
                                                </td>
                                                <td style="padding: 12px; border: 1px solid #ddd; font-size: 0.85em; color: #666;">{inst.last_sync}</td>
                                            </tr>
                                        }
                                    }).collect_view()
                                },
                                Err(e) => view! { <tr><td colspan="5" style="color: #c00; padding: 20px; text-align: center;">{format!("Data Error: {}", e)}</td></tr> }.into_view()
                            }
                        })}
                    </Transition>
                </tbody>
            </table>
        </div>
    }
}

async fn fetch_health_data() -> Result<Vec<HealthInstance>, String> {
    // Relative path for GitHub Pages
    let url = "./dashboard_data.json"; 
    let resp = Request::get(url).send().await.map_err(|e| e.to_string())?;
    
    if !resp.ok() { 
        return Err(format!("Server Error: HTTP {}", resp.status())); 
    }
    
    resp.json::<Vec<HealthInstance>>()
        .await
        .map_err(|e| format!("JSON Error: {}", e))
}

fn main() { 
    mount_to_body(|| view! { <App /> }) 
}
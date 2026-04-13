use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    #[serde(default)] pub customer_name: Option<String>,
    #[serde(default)] pub host_name: Option<String>,
    #[serde(default, alias = "instance_name")] pub group: Option<String>,
    #[serde(default)] pub status: Option<String>,
    #[serde(default, alias = "timestamp")] pub last_sync: Option<String>,
}

#[component]
fn App() -> impl IntoView {
    let (search_query, set_search_query) = create_signal(String::new());
    let (selected_customer, set_selected_customer) = create_signal(None::<String>);
    
    let health_data = create_resource(move || (), |_| async move { fetch_health_data().await });

    view! {
        <div style="padding: 20px; font-family: sans-serif; background-color: #f0f4f8; min-height: 100vh;">
            <div style="max-width: 1200px; margin: auto; background: white; padding: 25px; border-radius: 12px; box-shadow: 0 4px 15px rgba(0,0,0,0.1);">
                
                <h2 style="color: #004488; margin-bottom: 20px;">"JDE Global Health Monitor"</h2>

                // Breadcrumb / Back Button
                {move || selected_customer.get().map(|name| view! {
                    <button 
                        on:click=move |_| set_selected_customer.set(None)
                        style="background: #004488; color: white; border: none; padding: 8px 15px; border-radius: 5px; cursor: pointer; margin-bottom: 20px;"
                    >
                        "← Back to All Customers"
                    </button>
                    <h3 style="margin-bottom: 15px;">"Customer: " {name}</h3>
                })}

                <input 
                    type="text" 
                    placeholder="Search..." 
                    style="width: 100%; padding: 12px; margin-bottom: 20px; border: 1px solid #ccd; border-radius: 8px;"
                    on:input=move |ev| set_search_query.set(event_target_value(&ev))
                    prop:value=search_query
                />

                <Transition fallback=move || view! { <p>"Loading Health Data..."</p> }>
                    {move || health_data.get().map(|data| match data {
                        Ok(instances) => {
                            let query = search_query.get().to_lowercase();
                            
                            if let Some(customer) = selected_customer.get() {
                                // DETAIL VIEW: Show instances for the selected customer
                                render_detail_view(instances, customer, query)
                            } else {
                                // SUMMARY VIEW: Show list of unique customers
                                render_summary_view(instances, query, set_selected_customer)
                            }
                        },
                        Err(e) => view! { <p style="color: red;">"Error: " {e}</p> }.into_view()
                    })}
                </Transition>
            </div>
        </div>
    }
}

// Renders the high-level list of unique customers
fn render_summary_view(instances: Vec<HealthInstance>, query: String, set_selected: WriteSignal<Option<String>>) -> View {
    let mut customers = HashMap::new();
    for inst in instances {
        let name = inst.customer_name.clone().unwrap_or_else(|| "Unknown".into());
        if name.to_lowercase().contains(&query) {
            *customers.entry(name).or_insert(0) += 1;
        }
    }

    let mut sorted_customers: Vec<_> = customers.into_iter().collect();
    sorted_customers.sort_by(|a, b| a.0.cmp(&b.0));

    view! {
        <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(250px, 1fr)); gap: 15px;">
            {sorted_customers.into_iter().map(|(name, count)| {
                let display_name = name.clone();
                view! {
                    <div 
                        on:click=move |_| set_selected.set(Some(name.clone()))
                        style="padding: 20px; border: 1px solid #ddd; border-radius: 10px; cursor: pointer; background: #fafafa; transition: transform 0.2s;"
                        on:mouseover=move |e| { let _ = e; } // Add hover effects in real CSS
                    >
                        <h4 style="margin: 0; color: #004488;">{display_name}</h4>
                        <p style="margin: 5px 0 0 0; font-size: 0.9em; color: #666;">{count} " Instances"</p>
                    </div>
                }
            }).collect_view()}
        </div>
    }.into_view()
}

// Renders the detailed table for a specific customer
fn render_detail_view(instances: Vec<HealthInstance>, customer: String, query: String) -> View {
    let filtered: Vec<_> = instances.into_iter()
        .filter(|inst| inst.customer_name.as_ref() == Some(&customer))
        .filter(|inst| inst.group.as_deref().unwrap_or("").to_lowercase().contains(&query))
        .collect();

    view! {
        <table style="width: 100%; border-collapse: collapse;">
            <thead>
                <tr style="background-color: #004488; color: white; text-align: left;">
                    <th style="padding: 12px;">"Host"</th>
                    <th style="padding: 12px;">"Instance"</th>
                    <th style="padding: 12px;">"Status"</th>
                    <th style="padding: 12px;">"Last Update"</th>
                </tr>
            </thead>
            <tbody>
                {filtered.into_iter().map(|inst| {
                    let status_str = inst.status.clone().unwrap_or_else(|| "UNKNOWN".into());
                    let is_ok = status_str == "RUNNING" || status_str == "Passed";
                    let (bg, fg) = if is_ok { ("#e6fffa", "#234e52") } else { ("#fff5f5", "#742a2a") };
                    view! {
                        <tr style="border-bottom: 1px solid #edf2f7;">
                            <td style="padding: 12px;">{inst.host_name.clone().unwrap_or_default()}</td>
                            <td style="padding: 12px;">{inst.group.clone().unwrap_or_default()}</td>
                            <td style="padding: 12px;">
                                <span style=format!("padding: 4px 10px; border-radius: 20px; font-weight: bold; background: {}; color: {};", bg, fg)>
                                    {status_str}
                                </span>
                            </td>
                            <td style="padding: 12px; font-size: 0.8em; color: #666;">{inst.last_sync.clone().unwrap_or_default()}</td>
                        </tr>
                    }
                }).collect_view()}
            </tbody>
        </table>
    }.into_view()
}

async fn fetch_health_data() -> Result<Vec<HealthInstance>, String> {
    let url = format!("https://e1cnc.github.io/jde-health-dashboard/dashboard_data.json?v={}", js_sys::Math::random()); 
    let resp = Request::get(&url).send().await.map_err(|e| e.to_string())?;
    resp.json::<Vec<HealthInstance>>().await.map_err(|e| e.to_string())
}

fn main() { mount_to_body(|| view! { <App /> }) }
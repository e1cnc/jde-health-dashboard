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
                
                <h2 style="color: #004488; margin-bottom: 20px;">"JDE Global Health Monitor Dashboard"</h2>

                {move || selected_customer.get().map(|name| view! {
                    <button 
                        on:click=move |_| {
                            set_selected_customer.set(None);
                            set_search_query.set(String::new()); // Reset search on back
                        }
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
                                render_detail_view(instances, customer, query)
                            } else {
                                render_summary_view(instances, query, set_selected_customer, set_search_query)
                            }
                        },
                        Err(e) => view! { <p style="color: red;">"Error: " {e}</p> }.into_view()
                    })}
                </Transition>
            </div>
        </div>
    }
}

fn render_summary_view(
    instances: Vec<HealthInstance>, 
    query: String, 
    set_selected: WriteSignal<Option<String>>,
    set_search: WriteSignal<String>
) -> View {
    // Map of Customer Name -> (Total Count, Has Critical Error)
    let mut customers: HashMap<String, (i32, bool)> = HashMap::new();
    
    for inst in instances {
        let name = inst.customer_name.clone().unwrap_or_else(|| "Unknown".into());
        let status = inst.status.as_deref().unwrap_or("UNKNOWN").to_uppercase();
        let is_critical = status == "STOPPED" || status == "FAILED";

        let entry = customers.entry(name).or_insert((0, false));
        entry.0 += 1;
        if is_critical { entry.1 = true; }
    }

    let mut sorted_customers: Vec<_> = customers.into_iter()
        .filter(|(name, _)| name.to_lowercase().contains(&query))
        .collect();
    sorted_customers.sort_by(|a, b| a.0.cmp(&b.0));

    view! {
        <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 15px;">
            {sorted_customers.into_iter().map(|(name, (count, has_error))| {
                let display_name = name.clone();
                let bg_color = if has_error { "#fff5f5" } else { "#fafafa" };
                let border_color = if has_error { "#feb2b2" } else { "#ddd" };
                let status_text = if has_error { "● CRITICAL ISSUES" } else { "● Healthy" };
                let status_color = if has_error { "#c53030" } else { "#38a169" };

                view! {
                    <div 
                        on:click=move |_| {
                            set_selected.set(Some(name.clone()));
                            set_search.set(String::new()); // Reset search when entering
                        }
                        style=format!(
                            "padding: 20px; border: 2px solid {}; border-radius: 10px; cursor: pointer; background: {}; transition: transform 0.1s;",
                            border_color, bg_color
                        )
                    >
                        <h4 style="margin: 0; color: #004488;">{display_name}</h4>
                        <div style="display: flex; justify-content: space-between; align-items: center; margin-top: 10px;">
                            <span style="font-size: 0.85em; color: #666;">{count} " Instances"</span>
                            <span style=format!("font-size: 0.75em; font-weight: bold; color: {};", status_color)>
                                {status_text}
                            </span>
                        </div>
                    </div>
                }
            }).collect_view()}
        </div>
    }.into_view()
}

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
                                <span style=format!("padding: 4px 10px; border-radius: 20px; font-weight: bold; background: {}; color: {}; border: 1px solid {};", bg, fg, fg)>
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
    
    // Safety check for empty or null data
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if text.is_empty() || text == "null" { return Ok(vec![]); }
    
    serde_json::from_str::<Vec<HealthInstance>>(&text).map_err(|e| format!("JSON Parse Error: {}", e))
}

fn main() { mount_to_body(|| view! { <App /> }) }
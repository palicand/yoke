use leptos::prelude::*;

use super::window_chrome::WindowChrome;

#[component]
pub fn AppShell(children: Children) -> impl IntoView {
    view! {
        <div class="qs-app">
            <WindowChrome/>
            <div class="qs-shell">
                {children()}
            </div>
        </div>
    }
}

use leptos::prelude::*;

use super::left_rail::LeftRail;
use super::toast::Toast;
use super::window_chrome::WindowChrome;

#[component]
pub fn AppShell(children: Children) -> impl IntoView {
    view! {
        <div class="qs-app">
            <WindowChrome/>
            <div class="qs-shell">
                <LeftRail/>
                <main class="qs-main">{children()}</main>
            </div>
            <Toast/>
        </div>
    }
}

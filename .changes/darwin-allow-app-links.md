---
"wry": minor
---

On macOS and iOS, add `WebViewBuilderExtDarwin::with_allow_app_links`. Set it to `false` so that WebKit does not hand allowed navigations to another application that claims the URL (app links and universal links).

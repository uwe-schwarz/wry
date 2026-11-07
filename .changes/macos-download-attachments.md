---
"wry": patch
---

On macOS and iOS, download responses with `Content-Disposition: attachment` through the download handler, like browsers do, instead of displaying them when WebKit can show their type (for example PDF or plain text).

// Thin wrappers over the global Tauri API (enabled by app.withGlobalTauri).
const T = window.__TAURI__;
export const invoke = (cmd, args) => T.core.invoke(cmd, args);
export const listen = (name, cb) => T.event.listen(name, (e) => cb(e.payload));
export const appWindow = T.window.getCurrentWindow();
export const pickFile = (filters) => T.dialog.open({ multiple: false, filters });

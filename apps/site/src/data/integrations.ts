export type Integration = {
  id: string; name: string; tag: string; title: string; description: string;
  language: string; code: string; meta: string[]; note: string;
};

export const INTEGRATIONS: Integration[] = [
  {
    id: "tauri", name: "Tauri", tag: "rust", title: "Keep permission in the native host",
    description: "Expose a narrow command. Run discovery off the UI thread after a user action.",
    language: "Rust: src-tauri", meta: ["host command", "passive discovery", "owned lifetime"],
    note: "Source preview: add the originlm path dependency and register this command in your Tauri builder. Keep execution policy in Rust.",
    code: `#[tauri::command]
async fn ai_snapshot() -> Result<originlm::Snapshot, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut ai = originlm::LocalAi::start()
            .map_err(|error| error.to_string())?;
        ai.scan().map_err(|error| error.to_string())
        // Drop closes this owned client, never another app's work.
    })
    .await
    .map_err(|error| error.to_string())?
}`,
  },
  {
    id: "electron", name: "Electron", tag: "node", title: "Keep the helper in the main process",
    description: "The dependency-first client selects a granted broker or its own packaged helper.",
    language: "JavaScript: main process", meta: ["native helper", "main process", "explicit cleanup"],
    note: "Use this from your existing app after an explicit Connect action. Unpack the helper package from asar; do not expose arbitrary RPC or executable paths to the renderer.",
    code: `import { app } from "electron";
import { createLocalAI } from "originlm";

await app.whenReady();
// Call from your app's trusted Connect handler.
async function inspectLocalAI() {
    const ai = await createLocalAI();
    try {
        return await ai.snapshot.get();
    } finally {
        await ai.close();
    }
}`,
  },
  {
    id: "gpui", name: "GPUI", tag: "rust", title: "Native UI, one canonical runtime",
    description: "A worker obtains a snapshot; your view renders the evidence. No provider logic enters the UI.",
    language: "Rust: worker function", meta: ["native", "background work", "typed snapshot"],
    note: "Run this function on GPUI's background executor from a user action, then update your view on its context. The desktop app in this repository is the complete native consumer.",
    code: `use originlm::{LocalAi, Snapshot};

fn inspect_local_ai() -> Result<Snapshot, String> {
    let mut ai = LocalAi::start()
        .map_err(|error| error.to_string())?;
    ai.scan().map_err(|error| error.to_string())
}

// Inside your existing GPUI view's refresh handler:
let task = cx.background_executor().spawn(async {
    inspect_local_ai()
});
// Await task in cx.spawn, then update the view with its result.`,
  },
  {
    id: "node", name: "Node", tag: "node", title: "Start with one client",
    description: "Read the current evidence through the public SDK. The application owns the client's lifetime.",
    language: "JavaScript: Node", meta: ["shared broker", "private fallback", "typed SDK"],
    note: "Install the local SDK and matching helper package, or set LAI_HELPER_PATH to your source-built executable. Protocol mismatch may fall back; denied authorization remains an error.",
    code: `import { createLocalAI } from "originlm";

const ai = await createLocalAI();
try {
    const snapshot = await ai.snapshot.get();
    console.log(ai.mode, snapshot.revision);
} finally {
    await ai.close();
}`,
  },
  {
    id: "python", name: "Python", tag: "python", title: "The same protocol, a context manager",
    description: "The Python client uses the packaged helper and releases owned resources on exit.",
    language: "Python", meta: ["automatic resolution", "context manager", "inert imports"],
    note: "Use a locally prepared wheel or set LAI_HELPER_PATH. AsyncClient.auto() is the async form. Extra metadata and execution require separate grants.",
    code: `from originlm import Client

with Client.auto() as ai:
    snapshot = ai.snapshot.get()
    print(ai.mode, snapshot["revision"])

# No scanning, spawning or credential reads occur on import.`,
  },
  {
    id: "web", name: "Web", tag: "web", title: "An explicit browser boundary",
    description: "An installed extension page can use Native Messaging. The local dashboard supports same-origin pairing.",
    language: "JavaScript: extension page", meta: ["Native Messaging", "registered identity", "explicit connection"],
    note: "Register com.originlm.host for the exact extension identity. Arbitrary websites have no content bridge yet; cross-origin loopback access is not enabled.",
    code: `import { createLocalAI, createNativeMessagingTransport }
    from "originlm";

// In the registered extension, after the user's Connect action.
const transport = createNativeMessagingTransport({
    connect: () => chrome.runtime.connectNative("com.originlm.host"),
});
const ai = await createLocalAI({ transport });
try {
    const snapshot = await ai.snapshot.get();
    console.log(snapshot.revision);
} finally {
    await ai.close();
}`,
  },
];

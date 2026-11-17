import { test, expect } from "@playwright/test";
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    let next = 1;
    const colors = { background: "#101010", panel: "#181818", bar: "#252525", barText: "#EEEEEE", button: "#292929", buttonText: "#EEEEEE", text: "#EEEEEE", accent: "#F2CD65" };
    const plugins: unknown[] = [];
    (window as any).__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
      transformCallback: () => next++, unregisterCallback: () => {},
      invoke: async (command: string) => {
        if (command === "get_settings") return { language: "en", theme: { preset: "concept", colors }, workspace: { trackPanel:true, pianoRoll:true, terminal:true, pxPerSec:60, trackPanelWidth:300, bottomHeight:420, terminalWidth:511 }, recentProjects:[] };
        if (command === "get_app_info") return { version:"test", engineRunning:false };
        if (command === "list_voices") return [{id:"sapi:test",name:"Test speech voice",engine:"sapi",languages:["en-US"],supportsPitch:true}];
        if (command === "synthesize") return {path:"C:/generated/test.wav",durationSec:1.5,sampleRate:22050};
        if (command === "list_instruments") return structuredClone(plugins);
        if (command === "install_instrument") { plugins.push({ id:"test-bell", name:"Test Bell", version:1 }); return plugins[0]; }
        if (command === "plugin:dialog|open") return "C:/example.py";
        if (command === "run_python_preset") return { name:"Test arpeggio", tempo:120, notes:[{ midi:60,start:0,duration:1,lyric:"la" }] };
        if (command === "voice_catalog") return [{id:"test",name:"Test Voice",language:"en-US",bytes:114199011,modelCard:"https://example.com",datasetLicense:"Test license",installed:false}];
        if (command === "plugin:window|inner_size") return {width:1600,height:900};
        if (command === "plugin:event|listen") return next++;
        return null;
      }
    };
    (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
  });
  await page.goto("/");
});
async function settingsItem(page: any, label: string) {
  await page.getByRole("button", {name:"Settings", exact:true}).click();
  await page.getByText(label, {exact:true}).click();
}
test("instrument installation appears in piano-roll selector", async ({page}) => {
  await settingsItem(page,"Extensions");
  await page.getByRole("button",{name:"Install instrument…"}).click();
  await expect(page.getByText("Instrument installed.")).toBeVisible();
  await page.getByRole("dialog").getByRole("button",{name:"Close",exact:true}).click();
  await expect(page.locator('.roll-toolbar select').first()).toContainText("Test Bell");
  await page.locator('.roll-toolbar select').first().selectOption("plugin:test-bell");
  await expect(page.locator('.roll-toolbar select').first()).toHaveValue("plugin:test-bell");
});
test("Python preset requires trust, previews, applies and can be undone", async ({page}) => {
  await settingsItem(page,"Extensions");
  const run = page.getByRole("button",{name:"Run script",exact:true});
  await expect(run).toBeDisabled();
  await page.getByRole("button",{name:"Choose .py…"}).click();
  await expect(run).toBeDisabled();
  await page.getByRole("checkbox").check(); await run.click();
  await expect(page.getByText("1 notes ready to add as a new clip.")).toBeVisible();
  await page.getByRole("button",{name:"Apply · undoable"}).click();
  await page.getByRole("dialog").getByRole("button",{name:"Close",exact:true}).click();
  await expect(page.locator('.note-rect')).toHaveCount(1);
  await page.keyboard.press("Control+z");
  await expect(page.locator('.note-rect')).toHaveCount(0);
});
test("voice catalogue explains runtime and local import", async ({page}) => {
  await settingsItem(page,"Voice catalogue");
  await expect(page.getByRole("heading",{name:"Voice catalogue"})).toBeVisible();
  await expect(page.getByText("python -m pip install piper-tts")).toBeVisible();
  await expect(page.getByRole("button",{name:"Import .onnx…"})).toBeEnabled();
  await page.screenshot({path:"test-results/voice-catalog.png"});
});

test("speech generation creates an undoable WAV clip", async ({page}) => {
  await page.getByRole("button", {name:"Speech synthesis",exact:true}).click();
  await page.getByRole("textbox",{name:"Type dialogue text here..."}).fill("Hello from Energy");
  await page.getByRole("button",{name:"Generate",exact:true}).click();
  await expect(page.getByText("1.50 s")).toBeVisible();
  await page.getByRole("button",{name:"Add to timeline"}).click();
  await expect(page.getByText("Speech clip added. Undo is available.")).toBeVisible();
  await page.getByRole("dialog").getByRole("button",{name:"Close",exact:true}).click();
  await expect(page.locator('.clip').filter({hasText:"Hello from Energy"})).toHaveCount(1);
  await page.keyboard.press("Control+z");
  await expect(page.locator('.clip').filter({hasText:"Hello from Energy"})).toHaveCount(0);
});

test("transport fits the minimum supported window", async ({page}) => {
  await page.setViewportSize({width:1280,height:720});
  const exportButton = page.getByRole("button",{name:"Export",exact:true});
  const bounds = await exportButton.boundingBox();
  expect(bounds).not.toBeNull();
  expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(1280);
  await expect(page.getByRole("button",{name:"Speech synthesis",exact:true})).toBeVisible();
});

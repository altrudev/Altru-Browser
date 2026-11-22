#!/usr/bin/env node
// Records the UI operations a PWX Strict app sends, so Altru Browser can replay them
// through its own engine before a JavaScript engine is wired in.
//
// It runs the app exactly as the PWX Strict runtime does: the real PwxUI prelude
// (read from a PWX checkout's app-origin/renderer.js, so there is one source of truth),
// then the app's own code, in a Node vm context with no DOM, no fetch and no network.
//
// Usage:
//   node tools/pwx-strict-record.mjs <pwx-checkout>/spike/app-origin/renderer.js \
//        <pwx-checkout>/spike/apps/notes/main.js [--storage] [--seed seed.json] \
//        [--click "Button label"]... [--type "Field label=text"]... > out.strict
//
// Output: one operation per line, tab-separated, `\t` `\n` `\\` escaped.
// A line `frame` separates each batch the app posted. A line `event <id> <type> <value>`
// records a user action delivered to the app (clicks, typing).

import { readFileSync } from "node:fs";
import vm from "node:vm";
import { randomUUID } from "node:crypto";

const args = process.argv.slice(2);
const [rendererPath, appPath] = args;
if (!rendererPath || !appPath) {
  console.error("usage: pwx-strict-record.mjs <renderer.js> <app.js> [--storage] [--seed f] [--click label] [--type label=text]");
  process.exit(2);
}
const flag = (name) => args.includes(name);
const many = (name) => args.flatMap((a, i) => (a === name ? [args[i + 1]] : []));

const renderer = readFileSync(rendererPath, "utf8");
const preludeMatch = renderer.match(/const PRELUDE = `([\s\S]*?)`;/);
const storeMatch = renderer.match(/const STORE_API = `([\s\S]*?)`;/);
if (!preludeMatch || !storeMatch) {
  console.error("could not find PRELUDE / STORE_API in renderer.js");
  process.exit(1);
}
const prelude = preludeMatch[1].replace("/*STORE*/", flag("--storage") ? storeMatch[1] : "");
const seed = many("--seed")[0] ? JSON.parse(readFileSync(many("--seed")[0], "utf8")) : [];
// The app's own data, in memory only, standing in for the runtime's per-app store.
const kv = new Map(seed.map((n) => [n.id, n]));

const frames = [];
const listeners = new Map();
const sandbox = {
  console,
  queueMicrotask,
  setTimeout: (fn) => { pending.push(fn); return pending.length; },
  clearTimeout: () => {},
  crypto: { randomUUID },
  Date: { now: (() => { let t = 1_700_000_000_000; return () => (t += 1000); })() },
  postMessage: (message) => {
    if (Array.isArray(message.ops)) frames.push(message.ops);
    if (message.t === "store") {
      const { rid, op, key, value } = message;
      const result = op === "get" ? kv.get(key) : op === "put" ? (kv.set(key, value), undefined)
        : op === "delete" ? (kv.delete(key), undefined) : op === "all" ? [...kv.values()] : undefined;
      queueMicrotask(() => emit({ t: "store-result", rid, ok: true, value: structuredClone(result) }));
    }
  },
  addEventListener: (type, fn) => { if (type === "message") listeners.set(fn, fn); },
};
const pending = [];
sandbox.self = sandbox;
const emit = (data) => { for (const fn of listeners.values()) fn({ data }); };
const settle = async () => {
  for (let i = 0; i < 20; i++) {
    await new Promise((r) => setImmediate(r));
    while (pending.length) pending.shift()();
  }
};

vm.createContext(sandbox);
vm.runInContext(prelude + "\n;\n" + readFileSync(appPath, "utf8"), sandbox, { filename: appPath });
await settle();

// Find the current node id for a button by its text, or a field by aria-label/placeholder.
function find(predicate) {
  const state = new Map();
  for (const op of frames.flat()) {
    if (op.t === "create") state.set(op.id, { tag: op.tag, attrs: {}, text: "" });
    else if (op.t === "attr" && state.has(op.id)) state.get(op.id).attrs[op.name] = op.value;
    else if (op.t === "text" && state.has(op.id)) state.get(op.id).text = op.value;
    else if (op.t === "remove") state.delete(op.id);
  }
  return [...state.entries()].reverse().find(([, n]) => predicate(n))?.[0];
}

for (let i = 0; i < args.length; i++) {
  if (args[i] === "--click") {
    const label = args[++i];
    const id = find((n) => n.tag === "button" && n.text === label);
    if (id === undefined) { console.error(`no button "${label}"`); process.exit(1); }
    frames.push([{ t: "__event", id, type: "click", value: "" }]);
    emit({ t: "event", id, type: "click", value: null, key: null });
    await settle();
  } else if (args[i] === "--type") {
    const [label, ...rest] = args[++i].split("=");
    const text = rest.join("=");
    const id = find((n) => n.attrs["aria-label"] === label || n.attrs.placeholder === label);
    if (id === undefined) { console.error(`no field "${label}"`); process.exit(1); }
    // What the user typed lands in the field itself, as in a real browser.
    frames.push([{ t: "__event", id, type: "input", value: text }]);
    emit({ t: "event", id, type: "input", value: text, key: null });
    await settle();
  }
}

const esc = (v) => String(v).replace(/\\/g, "\\\\").replace(/\t/g, "\\t").replace(/\n/g, "\\n");
const lines = [];
for (const ops of frames) {
  for (const op of ops) {
    switch (op.t) {
      case "create": lines.push(["create", op.id, op.tag]); break;
      case "attr": lines.push(["attr", op.id, op.name, op.value]); break;
      case "text": lines.push(["text", op.id, op.value]); break;
      case "prop": lines.push(["prop", op.id, op.name, typeof op.value === "boolean" ? `b:${op.value}` : `s:${op.value}`]); break;
      case "append": lines.push(["append", op.parent, op.id]); break;
      case "remove": lines.push(["remove", op.id]); break;
      case "focus": lines.push(["focus", op.id]); break;
      case "listen": lines.push(["listen", op.id, op.type]); break;
      case "__event": lines.push(["event", op.id, op.type, op.value]); break;
      default: lines.push(["unknown", op.t]);
    }
  }
  if (ops.some((op) => op.t !== "__event")) lines.push(["frame"]);
}
process.stdout.write(lines.map((l) => l.map(esc).join("\t")).join("\n") + "\n");

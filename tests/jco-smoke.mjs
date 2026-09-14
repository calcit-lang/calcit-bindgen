import { pathToFileURL } from "node:url";

const modulePath = process.argv[2];
if (modulePath === undefined) {
  throw new Error("usage: node tests/jco-smoke.mjs <transpiled-component.js>");
}

const { instantiate } = await import(pathToFileURL(modulePath));
const api = await instantiate(undefined, {
  host: {
    addOne: (value) => value + 2,
    boolNot: (value) => !value,
    echo: (value) => value,
  },
});

if (api.addOne(41) !== 42) throw new Error("jco addOne smoke failed");
if (api.callHostAddOne(40) !== 42) throw new Error("jco callHostAddOne smoke failed");
if (api.boolNot(true) !== false || api.boolNot(false) !== true) {
  throw new Error("jco boolNot smoke failed");
}
if (api.callHostBoolNot(true) !== false || api.callHostBoolNot(false) !== true) {
  throw new Error("jco callHostBoolNot smoke failed");
}
if (api.chooseNumber(true, 3, 4) !== 3 || api.chooseNumber(false, 3, 4) !== 4) {
  throw new Error("jco mixed Bool/Number smoke failed");
}
if (api.echoText("Calcit") !== "Calcit") throw new Error("jco echoText smoke failed");
if (api.callHostEcho("Agent") !== "Agent") throw new Error("jco callHostEcho smoke failed");

console.log("jco Bool/Number/String smoke passed");

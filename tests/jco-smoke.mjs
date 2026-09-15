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
    buffer: (value) => Uint8Array.from(value).reverse(),
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
const expectBytes = (actual, expected, label) => {
  if (actual.length !== expected.length || actual.some((byte, index) => byte !== expected[index])) {
    throw new Error(`${label}: got [${actual}], expected [${expected}]`);
  }
};
for (const bytes of [[], [0, 255, 17], [128, 0, 254, 1]]) {
  expectBytes(api.echoBuffer(Uint8Array.from(bytes)), bytes, "jco echoBuffer smoke failed");
}
const yesBuffer = Uint8Array.of(0, 255);
const noBuffer = Uint8Array.of(17, 0, 128);
expectBytes(api.chooseBuffer(true, yesBuffer, noBuffer), [0, 255], "jco chooseBuffer true failed");
expectBytes(api.chooseBuffer(false, yesBuffer, noBuffer), [17, 0, 128], "jco chooseBuffer false failed");
if (api.isBuffer(Uint8Array.of(0, 255, 17)) !== true) throw new Error("jco isBuffer smoke failed");
expectBytes(
  api.callHostBuffer(Uint8Array.of(0, 255, 17)),
  [17, 255, 0],
  "jco callHostBuffer smoke failed",
);
if (api.chooseNumber(true, 3, 4) !== 3 || api.chooseNumber(false, 3, 4) !== 4) {
  throw new Error("jco mixed Bool/Number smoke failed");
}
if (api.echoText("Calcit") !== "Calcit") throw new Error("jco echoText smoke failed");
if (api.callHostEcho("Agent") !== "Agent") throw new Error("jco callHostEcho smoke failed");

console.log("jco Bool/Buffer/Number/String smoke passed");

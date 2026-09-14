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
    numbers: (value) => Float64Array.from(value).reverse(),
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

const expectList = (actual, expected, label) => {
  const value = Array.from(actual);
  if (JSON.stringify(value) !== JSON.stringify(expected)) {
    throw new Error(`${label}: got ${JSON.stringify(value)}, expected ${JSON.stringify(expected)}`);
  }
};
expectList(api.echoBools([true, false, true]), [true, false, true], "jco Bool List smoke failed");
expectList(api.echoNumbers(Float64Array.of(1, -2.5, 7)), [1, -2.5, 7], "jco Number List smoke failed");
expectList(api.callHostNumbers(Float64Array.of(1, -2.5, 7)), [7, -2.5, 1], "jco host Number List smoke failed");
expectList(api.echoTexts(["alpha", "", "世界"]), ["alpha", "", "世界"], "jco String List smoke failed");
const buffers = api.echoBuffers([Uint8Array.of(0, 255), Uint8Array.of(), Uint8Array.of(17, 0, 128)]);
if (buffers.length !== 3) throw new Error(`jco Buffer List smoke returned ${buffers.length} items`);
expectBytes(buffers[0], [0, 255], "jco Buffer List item 0 failed");
expectBytes(buffers[1], [], "jco Buffer List item 1 failed");
expectBytes(buffers[2], [17, 0, 128], "jco Buffer List item 2 failed");
const nested = api.echoNumberLists([Float64Array.of(1, 2), Float64Array.of(), Float64Array.of(-3, 4.5, 7)]);
if (nested.length !== 3) throw new Error(`jco nested Number List smoke returned ${nested.length} items`);
expectList(nested[0], [1, 2], "jco nested Number List item 0 failed");
expectList(nested[1], [], "jco nested Number List item 1 failed");
expectList(nested[2], [-3, 4.5, 7], "jco nested Number List item 2 failed");

console.log("jco recursive List and scalar smoke passed");

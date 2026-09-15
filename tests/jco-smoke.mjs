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
    event: (value) => value,
    numbers: (value) => Float64Array.from(value).reverse(),
    numericScalars: (value) => value,
    optionNumber: (value) => value,
    ping: () => undefined,
    profile: (value) => ({
      ...value,
      active: !value.active,
      stats: { score: value.stats.score + 1 },
    }),
    resultNumber: (value) => {
      if (value.tag === "ok") return value.val;
      throw { payload: value.val };
    },
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
const numericScalars = {
  f32: 1.5,
  f64: 1.25,
  i16: -32768,
  i32: -2147483648,
  i64: -9007199254740991n,
  i8: -128,
  u16: 65535,
  u32: 4294967295,
  u64: 9007199254740991n,
  u8: 255,
};
const expectNumericScalars = (actual, expected, label) => {
  for (const name of Object.keys(expected)) {
    if (actual[name] !== expected[name]) {
      throw new Error(`${label}: ${name} is ${actual[name]}, expected ${expected[name]}`);
    }
  }
};
expectNumericScalars(
  api.echoNumericScalars(numericScalars),
  numericScalars,
  "jco numeric Struct export smoke failed",
);
expectNumericScalars(
  api.callHostNumericScalars(numericScalars),
  numericScalars,
  "jco numeric Struct import smoke failed",
);
for (const [field, value] of [
  ["i64", -9007199254740992n],
  ["u64", 9007199254740992n],
]) {
  for (const [label, call] of [
    ["direct export", (input) => api.echoNumericScalars(input)],
    ["host import", (input) => api.callHostNumericScalars(input)],
  ]) {
    let trapped = false;
    try {
      call({ ...numericScalars, [field]: value });
    } catch {
      trapped = true;
    }
    if (!trapped) throw new Error(`jco unsafe ${field} ${label} must trap`);
  }
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

if (api.echoOptionNumber(undefined) !== undefined || api.echoOptionNumber(7.5) !== 7.5) {
  throw new Error("jco Option<Number> smoke failed");
}
if (api.echoOptionText(undefined) !== undefined || api.echoOptionText("你好") !== "你好") {
  throw new Error("jco Option<String> smoke failed");
}
if (api.callHostOptionNumber(undefined) !== undefined || api.callHostOptionNumber(12.5) !== 12.5) {
  throw new Error("jco host Option<Number> smoke failed");
}
const expectResultError = (invoke, expected, label) => {
  try {
    invoke();
  } catch (error) {
    if (error?.payload === expected) return;
    throw new Error(label + ": unexpected error " + error);
  }
  throw new Error(label + ": expected an error payload");
};
if (api.echoResultNumber({ tag: "ok", val: 9.25 }) !== 9.25) {
  throw new Error("jco Result<Number,String> ok smoke failed");
}
expectResultError(
  () => api.echoResultNumber({ tag: "err", val: "bad" }),
  "bad",
  "jco Result<Number,String> err smoke failed",
);
if (api.echoResultUnit({ tag: "ok", val: undefined }) !== undefined) {
  throw new Error("jco Result<Unit,String> ok smoke failed");
}
expectResultError(
  () => api.echoResultUnit({ tag: "err", val: "bad" }),
  "bad",
  "jco Result<Unit,String> err smoke failed",
);
expectList(
  api.echoResultNumbers({ tag: "ok", val: Float64Array.of(2, 4, 8) }),
  [2, 4, 8],
  "jco Result<List<Number>,String> ok smoke failed",
);
expectResultError(
  () => api.echoResultNumbers({ tag: "err", val: "bad" }),
  "bad",
  "jco Result<List<Number>,String> err smoke failed",
);
if (api.callHostResultNumber({ tag: "ok", val: 6.5 }) !== 6.5) {
  throw new Error("jco host Result<Number,String> ok smoke failed");
}
expectResultError(
  () => api.callHostResultNumber({ tag: "err", val: "bad" }),
  "bad",
  "jco host Result<Number,String> err smoke failed",
);
if (api.ping() !== undefined || api.callHostPing() !== undefined) {
  throw new Error("jco Unit smoke failed");
}

const profile = {
  active: true,
  maybeName: "Ada",
  name: "Ada",
  outcome: { tag: "ok", val: Float64Array.of(4, 5) },
  scores: Float64Array.of(1, 2, 3),
  stats: { score: 7.5 },
};
const expectProfile = (actual, expected, label) => {
  const normalizeResultValue = (value) => ArrayBuffer.isView(value) ? Array.from(value) : value;
  if (
    actual.active !== expected.active ||
    actual.maybeName !== expected.maybeName ||
    actual.name !== expected.name ||
    actual.outcome.tag !== expected.outcome.tag ||
    JSON.stringify(normalizeResultValue(actual.outcome.val)) !== JSON.stringify(normalizeResultValue(expected.outcome.val)) ||
    actual.stats.score !== expected.stats.score ||
    JSON.stringify(Array.from(actual.scores)) !== JSON.stringify(Array.from(expected.scores))
  ) {
    throw new Error(`${label}: got ${JSON.stringify(actual)}`);
  }
};
expectProfile(api.echoProfile(profile), profile, "jco Struct record export smoke failed");
expectProfile(
  api.callHostProfile(profile),
  { ...profile, active: false, stats: { score: 8.5 } },
  "jco Struct record import smoke failed",
);
const alternateProfile = {
  ...profile,
  maybeName: undefined,
  outcome: { tag: "err", val: "bad" },
};
expectProfile(
  api.echoProfile(alternateProfile),
  alternateProfile,
  "jco Struct record None/Err export smoke failed",
);
expectProfile(
  api.callHostProfile(alternateProfile),
  { ...alternateProfile, active: false, stats: { score: 8.5 } },
  "jco Struct record None/Err import smoke failed",
);

const expectEvent = (actual, expected, label) => {
  if (actual.tag !== expected.tag) {
    throw new Error(`${label}: got tag ${actual.tag}, expected ${expected.tag}`);
  }
  if (expected.tag === "idle") return;
  if (expected.tag === "profile") {
    expectProfile(actual.val, expected.val, label);
    return;
  }
  if (JSON.stringify(actual.val) !== JSON.stringify(expected.val)) {
    throw new Error(`${label}: got ${JSON.stringify(actual)}, expected ${JSON.stringify(expected)}`);
  }
};
for (const event of [
  { tag: "idle" },
  { tag: "moved", val: [3, 4] },
  { tag: "named", val: "Ada" },
  { tag: "profile", val: profile },
]) {
  expectEvent(api.echoEvent(event), event, "jco Enum export smoke failed");
  expectEvent(api.callHostEvent(event), event, "jco Enum import smoke failed");
}

console.log("jco numeric, Enum, Struct, Option/Result, recursive List, and scalar smoke passed");

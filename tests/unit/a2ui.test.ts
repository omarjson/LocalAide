import assert from "node:assert/strict";
import { test } from "node:test";
import {
  BASIC_CATALOG_ID,
  applyEnvelope,
  buildAction,
  buildSurfaces,
  callFunction,
  checkPattern,
  childEntries,
  errorReport,
  getPointer,
  localAction,
  mediaSource,
  parseStream,
  resolve,
  rootScope,
  runChecks,
  safePattern,
  setPointer,
  splitEventMessage,
  toBoolean,
  type A2uiProblem,
  type JsonObject,
} from "../../src/lib/a2ui.ts";

// The upstream spec test case specification/v1_0/test/cases/
// contact_form_example.jsonl, embedded so the test does not depend on the
// upstream checkout. String.raw keeps the pattern backslashes exact.
const CONTACT_FORM = String.raw`{"version": "1.0", "createSurface": {"surfaceId": "contact_form_1", "catalogId": "https://a2ui.org/specification/v1_0/catalogs/basic/catalog.json"}}
{"version": "1.0", "updateComponents": {"surfaceId": "contact_form_1", "components": [{"id": "root", "component": "Card", "child": "form_container"}, {"id": "form_container", "component": "Column", "children": ["header_row", "name_row", "email_group", "phone_group", "pref_group", "divider_1", "newsletter_checkbox", "submit_button"], "justify": "start", "align": "stretch"}, {"id": "header_row", "component": "Row", "children": ["header_icon", "header_text"], "align": "center"}, {"id": "header_icon", "component": "Icon", "name": "mail"}, {"id": "header_text", "component": "Text", "text": "# Contact Us"}, {"id": "name_row", "component": "Row", "children": ["first_name_group", "last_name_group"], "justify": "spaceBetween"}, {"id": "first_name_group", "component": "Column", "children": ["first_name_label", "first_name_field"], "weight": 1}, {"id": "first_name_label", "component": "Text", "text": "First Name", "variant": "caption"}, {"id": "first_name_field", "component": "TextField", "label": "First Name", "value": {"@path": "/contact/firstName"}, "variant": "shortText"}, {"id": "last_name_group", "component": "Column", "children": ["last_name_label", "last_name_field"], "weight": 1}, {"id": "last_name_label", "component": "Text", "text": "Last Name", "variant": "caption"}, {"id": "last_name_field", "component": "TextField", "label": "Last Name", "value": {"@path": "/contact/lastName"}, "variant": "shortText"}, {"id": "email_group", "component": "Column", "children": ["email_label", "email_field"]}, {"id": "email_label", "component": "Text", "text": "Email Address", "variant": "caption"}, {"id": "email_field", "component": "TextField", "label": "Email", "value": {"@path": "/contact/email"}, "variant": "shortText", "@checks": [{"condition": {"@call": "required", "args": {"value": {"@path": "/contact/email"}}}, "message": "Email is required."}, {"condition": {"@call": "email", "args": {"value": {"@path": "/contact/email"}}}, "message": "Please enter a valid email address."}]}, {"id": "phone_group", "component": "Column", "children": ["phone_label", "phone_field"]}, {"id": "phone_label", "component": "Text", "text": "Phone Number", "variant": "caption"}, {"id": "phone_field", "component": "TextField", "label": "Phone", "value": {"@path": "/contact/phone"}, "variant": "shortText", "@checks": [{"condition": {"@call": "regex", "args": {"value": {"@path": "/contact/phone"}, "pattern": "^\\d{10}$"}}, "message": "Phone number must be 10 digits."}]}, {"id": "pref_group", "component": "Column", "children": ["pref_label", "pref_picker"]}, {"id": "pref_label", "component": "Text", "text": "Preferred Contact Method", "variant": "caption"}, {"id": "pref_picker", "component": "ChoicePicker", "variant": "mutuallyExclusive", "options": [{"label": "Email", "value": "email"}, {"label": "Phone", "value": "phone"}, {"label": "SMS", "value": "sms"}], "value": {"@path": "/contact/preference"}}, {"id": "divider_1", "component": "Divider", "axis": "horizontal"}, {"id": "newsletter_checkbox", "component": "CheckBox", "label": "Subscribe to our newsletter", "value": {"@path": "/contact/subscribe"}}, {"id": "submit_button_label", "component": "Text", "text": "Send Message"}, {"id": "submit_button", "component": "Button", "child": "submit_button_label", "variant": "primary", "action": {"event": {"name": "submitContactForm", "context": {"formId": "contact_form_1", "clientTime": {"@call": "formatDate", "args": {"value": "2026-02-02T15:17:00Z", "format": "E MMM d, YYYY h:mm a"}}, "isNewsletterSubscribed": {"@path": "/contact/subscribe"}}}}}]}}
{"version": "1.0", "updateDataModel": {"surfaceId": "contact_form_1", "path": "/contact", "value": {"firstName": "John", "lastName": "Doe", "email": "john.doe@example.com", "phone": "1234567890", "preference": ["email"], "subscribe": true}}}
{"version": "1.0", "deleteSurface": {"surfaceId": "contact_form_1"}}`;

const create = (surfaceId = "s", components: unknown[] = []) =>
  JSON.stringify({ version: "1.0", createSurface: { surfaceId, catalogId: BASIC_CATALOG_ID, components } });

/** The error envelope inside an error report, parsed out of its fence. */
function errorEnvelope(report: string): Record<string, unknown> {
  const line = report.split("\n").find((l) => l.startsWith('{"version"'));
  assert.ok(line, "report carries an envelope");
  return JSON.parse(line) as Record<string, unknown>;
}

test("parseStream reads JSONL, arrays and single envelopes", () => {
  const jsonl = parseStream('{"version":"1.0","deleteSurface":{"surfaceId":"a"}}\n{"version":"1.0","deleteSurface":{"surfaceId":"b"}}');
  assert.equal(jsonl.envelopes.length, 2);
  assert.deepEqual(jsonl.problems, []);
  assert.equal(parseStream('[{"version":"1.0"}]').envelopes.length, 1);
  assert.equal(parseStream('{"version":"1.0"}').envelopes.length, 1);
  const empty = parseStream("  ");
  assert.equal(empty.problems[0]?.message, "The interface block is empty.");
  assert.equal(empty.problems[0]?.path, "/");
  const bad = parseStream('{"version":"1.0"}\nnot json\n{"version":"1.0"}');
  assert.equal(bad.envelopes.length, 2);
  assert.match(bad.problems[0]?.message ?? "", /Line 2 is not valid JSON/);
  assert.equal(bad.problems[0]?.path, "/");
});

test("the upstream contact-form stream builds with no problems", () => {
  const built = buildSurfaces(CONTACT_FORM.split("\n").slice(0, 3).join("\n"));
  assert.deepEqual(built.problems, []);
  assert.deepEqual(built.created, ["contact_form_1"]);
  const surface = built.surfaces.contact_form_1;
  assert.equal(Object.keys(surface.components).length, 25);
  assert.equal(surface.components.root.component, "Card");
  assert.deepEqual(surface.data, {
    contact: { firstName: "John", lastName: "Doe", email: "john.doe@example.com", phone: "1234567890", preference: ["email"], subscribe: true },
  });

  const scope = rootScope(surface.data);
  const emailField = surface.components.email_field;
  assert.deepEqual(runChecks(emailField, rootScope({ contact: { email: "" } })), { valid: false, message: "Email is required.", severity: "error" });
  assert.deepEqual(runChecks(emailField, rootScope({ contact: { email: "bad" } })), { valid: false, message: "Please enter a valid email address.", severity: "error" });
  assert.deepEqual(runChecks(emailField, scope), { valid: true });
  const phoneField = surface.components.phone_field;
  assert.equal(runChecks(phoneField, rootScope({ contact: { phone: "123" } })).valid, false);
  assert.deepEqual(runChecks(phoneField, scope), { valid: true });

  const action = buildAction(surface, surface.components.submit_button, scope, "Send Message", new Date("2026-02-02T15:17:00Z"));
  assert.ok(action, "the submit button builds an action");
  assert.equal(action?.text, 'I pressed “Send Message”.');
  const envelope = JSON.parse(action?.message.split("\n").find((l) => l.startsWith('{"version"')) ?? "{}") as Record<string, unknown>;
  const act = envelope.action as Record<string, unknown>;
  assert.equal(act.name, "submitContactForm");
  assert.equal(act.surfaceId, "contact_form_1");
  assert.equal(act.sourceComponentId, "submit_button");
  assert.equal(act.timestamp, "2026-02-02T15:17:00.000Z");
  const context = act.context as Record<string, unknown>;
  assert.equal(context.formId, "contact_form_1");
  assert.equal(context.isNewsletterSubscribed, true);
  assert.match(String(context.clientTime), /Feb.*2026/);

  const deleted = buildSurfaces(CONTACT_FORM.split("\n")[3], built.surfaces);
  assert.equal(deleted.surfaces.contact_form_1.deleted, true);
  assert.deepEqual(deleted.problems, []);
});

test("safePattern accepts ordinary patterns and the contact-form phone pattern", () => {
  assert.ok(safePattern("abc"));
  assert.ok(safePattern("^\\d{10}$"));
  assert.ok(safePattern("[a-z]+@[a-z]+"));
  assert.ok(safePattern("a".repeat(256)));
});

test("checkPattern enforces the 256 code-point cap", () => {
  assert.equal(checkPattern("a".repeat(257)).rejection, "PATTERN_TOO_LONG");
  assert.equal(checkPattern("a".repeat(256)).rejection, undefined);
});

test("checkPattern rejects patterns outside the dialect", () => {
  for (const pattern of ["(a)\\1", "(?<=a)b", "(?<!a)b", "(?<n>a)", "\\p{L}", "\\P{L}", "(?i)a", "\\01", "a\\k<name>"]) {
    assert.equal(checkPattern(pattern).rejection, "INVALID_PATTERN", pattern);
    assert.equal(checkPattern(pattern).regex, null);
  }
  // A literal backslash followed by "1" is not a backreference.
  assert.ok(safePattern("\\\\1"), "escaped backslash then 1 is a literal");
});

test("checkPattern rule 2: no quantifier above 1 over an unbounded quantifier", () => {
  for (const pattern of ["(a+)+", "(a+){2,3}", "((a+))+", "(a*){2,}", "(a+){2}"]) {
    assert.equal(checkPattern(pattern).rejection, "NESTED_QUANTIFIER", pattern);
  }
  for (const pattern of ["(a+)?", "(a+){1}", "(a+){0,1}", "(ab)+", "a+"]) {
    assert.equal(checkPattern(pattern).rejection, undefined, pattern);
    assert.ok(safePattern(pattern), pattern);
  }
});

test("checkPattern rule 3: no unbounded quantifier over a shared-first-char alternation", () => {
  for (const pattern of ["(a|ab)*c", "(a|ab)*", "(ab|ac)+", "(a|a)*", "x(a|ab)*y", "(?:a|ab)*", "((a|x)|a)*"]) {
    assert.equal(checkPattern(pattern).rejection, "OVERLAPPING_ALTERNATION", pattern);
  }
  for (const pattern of ["(a|b)*", "(a|b|c)*", "(a|ab)c", "((a|x)|b)*", "(ab|cd)+", "(a|b)?"]) {
    assert.equal(checkPattern(pattern).rejection, undefined, pattern);
    assert.ok(safePattern(pattern), pattern);
  }
});

test("checkPattern rule 4: no adjacent unbounded-quantified atoms with overlapping sets", () => {
  for (const pattern of ["\\d+\\s*\\d+", "\\d+\\s*\\w+", "\\s+\\s+", "\\d+\\w+", "\\d+(\\s*)\\d+", "[a-z]+[a-z]+", "\\w+\\d+", ".*\\d+", ".+a*", "\\d+\\s*\\D+"]) {
    assert.equal(checkPattern(pattern).rejection, "ADJACENT_OVERLAP", pattern);
  }
  for (const pattern of ["\\d+\\s+\\d+", "a+b+", "\\d+a+\\d+", "a*b*", "[a-z]+[0-9]+", "\\d+\\s+\\w+", "\\d+\\s*\\d", "\\d+ \\d+"]) {
    assert.equal(checkPattern(pattern).rejection, undefined, pattern);
    assert.ok(safePattern(pattern), pattern);
  }
});

test("regex reports an unsafe pattern as EXPRESSION_ERROR, not a silent non-match", () => {
  const scope = rootScope({});
  const unsafe = callFunction("regex", { value: "abc", pattern: "(a|ab)*" }, scope) as { valid: boolean; code?: string };
  assert.equal(unsafe.valid, false);
  assert.equal(unsafe.code, "EXPRESSION_ERROR");
  const dialect = callFunction("regex", { value: "abc", pattern: "(a)\\1" }, scope) as { valid: boolean; code?: string };
  assert.equal(dialect.code, "EXPRESSION_ERROR");
  const match = callFunction("regex", { value: "1234567890", pattern: "^\\d{10}$" }, scope) as { valid: boolean; code?: string };
  assert.deepEqual(match, { valid: true });
  const noMatch = callFunction("regex", { value: "123", pattern: "^\\d{10}$" }, scope) as { valid: boolean; code?: string };
  assert.equal(noMatch.valid, false);
  assert.equal(noMatch.code, undefined);
  const tooLong = callFunction("regex", { value: "x".repeat(4097), pattern: "x" }, scope) as { valid: boolean; code?: string };
  assert.equal(tooLong.valid, false);
  assert.equal(tooLong.code, undefined, "an over-long value is a silent non-match, never an error");
});

test("problems carry the JSON pointer of the failing field", () => {
  const badId = buildSurfaces(create("s", [{ id: "1bad", component: "Text", text: "x" }]));
  const surfaceProblems = badId.surfaces.s.problems;
  assert.equal(surfaceProblems[0]?.path, "/createSurface/components/0/id");
  assert.match(surfaceProblems[0]?.message ?? "", /not a plain name/);
  const report = errorReport("s", surfaceProblems);
  assert.equal((errorEnvelope(report).error as Record<string, unknown>).path, "/createSurface/components/0/id");

  const dup = buildSurfaces(create("s", []));
  const again = applyEnvelope(dup.surfaces, JSON.parse(create("s", [])) as Record<string, unknown>);
  assert.equal(again.problem?.path, "/createSurface/surfaceId");

  const missing = applyEnvelope({}, JSON.parse('{"version":"1.0","updateComponents":{"surfaceId":"nope","components":[]}}') as Record<string, unknown>);
  assert.equal(missing.problem?.path, "/updateComponents/surfaceId");
  const noValue = applyEnvelope(dup.surfaces, JSON.parse('{"version":"1.0","updateDataModel":{"surfaceId":"s","path":"/x"}}') as Record<string, unknown>);
  assert.equal(noValue.problem?.path, "/updateDataModel/value");
  const themed = applyEnvelope({}, JSON.parse(`{"version":"1.0","createSurface":{"surfaceId":"t","catalogId":"${BASIC_CATALOG_ID}","theme":{}}}`) as Record<string, unknown>);
  assert.equal(themed.problem?.path, "/createSurface/theme");

  const empty: A2uiProblem[] = [];
  assert.equal((errorEnvelope(errorReport("s", empty)).error as Record<string, unknown>).path, "/");
});

test("resolve, bindings and the catalog functions", () => {
  const scope = rootScope({ user: { name: "Omar", age: 30 }, items: ["a", "b"] });
  assert.equal(resolve({ "@path": "/user/name" }, scope), "Omar");
  assert.equal(resolve({ "@path": "user/age" }, scope), 30);
  assert.equal(resolve("plain", scope), "plain");
  assert.deepEqual(resolve([1, 2], scope), [1, 2]);
  assert.deepEqual(callFunction("required", { value: "" }, scope), { valid: false });
  assert.deepEqual(callFunction("required", { value: "x" }, scope), { valid: true });
  assert.deepEqual(callFunction("length", { value: "abc", min: 2, max: 4 }, scope), { valid: true });
  assert.deepEqual(callFunction("length", { value: "a", min: 2 }, scope), { valid: false });
  assert.deepEqual(callFunction("numeric", { value: "42", min: 0, max: 100 }, scope), { valid: true });
  assert.deepEqual(callFunction("numeric", { value: "nope" }, scope), { valid: false });
  assert.deepEqual(callFunction("email", { value: "a@b.co" }, scope), { valid: true });
  assert.deepEqual(callFunction("email", { value: "a@b" }, scope), { valid: false });
  assert.equal(callFunction("formatString", { value: "Hello ${/user/name}" }, scope), "Hello Omar");
  assert.equal(callFunction("formatNumber", { value: 1.5, decimals: 2 }, scope), "1.50");
  assert.equal(callFunction("pluralize", { value: 1, one: "item", other: "items" }, scope), "item");
  assert.equal(callFunction("pluralize", { value: 2, one: "item", other: "items" }, scope), "items");
  assert.equal(callFunction("and", { values: [true, { valid: true }, "true"] }, scope), true);
  assert.equal(callFunction("and", { values: [true, false] }, scope), false);
  assert.equal(callFunction("or", { values: [false, 0] }, scope), false);
  assert.equal(callFunction("or", { values: [0, "yes"] }, scope), false, "only 'true' counts as true for strings");
  assert.equal(callFunction("or", { values: [false, "true"] }, scope), true);
  assert.equal(callFunction("not", { value: 0 }, scope), true);
  assert.equal(callFunction("nope", {}, scope), undefined);
  assert.equal(toBoolean("true"), true);
  assert.equal(toBoolean("yes"), false);
  assert.equal(toBoolean(0), false);
  assert.equal(toBoolean({ valid: false }), false);
});

test("pointers, @index and child entries", () => {
  const data: JsonObject = {};
  setPointer(data, "/a/b/c", 7);
  assert.deepEqual(data, { a: { b: { c: 7 } } });
  assert.equal(getPointer(data, "/a/b/c"), 7);
  assert.deepEqual(getPointer(data, "/a/b"), { c: 7 });
  assert.equal(getPointer(data, "/a/nope"), undefined);
  setPointer(data, "/a/b/c", undefined);
  assert.deepEqual(data, { a: { b: {} } });
  assert.equal(getPointer({ a: { "~x": 1, "y/z": 2 } }, "/a/~0x"), 1);
  assert.equal(getPointer({ a: { "~x": 1, "y/z": 2 } }, "/a/y~1z"), 2);

  const scope = { data: { items: ["a", "b", "c"] }, base: "/items/1", index: 1 };
  assert.equal(resolve({ "@call": "@index", args: { offset: 1 } }, scope), 2);
  assert.equal(resolve({ "@call": "@index" }, { data: {}, base: "/", index: null }), undefined);
  const entries = childEntries({ componentId: "row", path: "/items" }, rootScope({ items: ["a", "b"] }));
  assert.equal(entries.length, 2);
  assert.equal(entries[1]?.scope.index, 1);
  assert.equal(entries[1]?.scope.base, "/items/1");
  assert.deepEqual(childEntries(["x", "y"], rootScope({})).map((e) => e.id), ["x", "y"]);
  assert.deepEqual(childEntries({ componentId: "row", path: "/nope" }, rootScope({})), []);
});

test("checks, actions, media and message splitting", () => {
  const scope = rootScope({ x: "" });
  const checked = {
    id: "f",
    component: "TextField",
    "@checks": [{ condition: { "@call": "required", args: { value: { "@path": "/x" } } }, message: "X required." }],
  };
  assert.deepEqual(runChecks(checked, scope), { valid: false, message: "X required.", severity: "error" });
  assert.deepEqual(runChecks(checked, rootScope({ x: "v" })), { valid: true });
  assert.deepEqual(runChecks({ id: "g", component: "Text" }, scope), { valid: true });

  const surface = buildSurfaces(create("s", [{ id: "b", component: "Button", action: { event: { name: "go" } } }])).surfaces.s;
  const plain = buildAction(surface, surface.components.b, rootScope({}), "Go");
  assert.ok(plain);
  assert.equal(plain?.text, 'I pressed “Go”.');
  const noAction = buildAction(surface, { id: "t", component: "Text", text: "hi" }, rootScope({}), "");
  assert.equal(noAction, null);

  const withUrl = { id: "u", component: "Button", action: { functionCall: { "@call": "openUrl", args: { url: "https://example.com" } } } };
  assert.deepEqual(localAction(withUrl, rootScope({})), { openUrl: "https://example.com/" });
  const badUrl = { id: "u", component: "Button", action: { functionCall: { "@call": "openUrl", args: { url: "javascript:alert(1)" } } } };
  assert.equal(localAction(badUrl, rootScope({})), null);
  const otherFn = { id: "u", component: "Button", action: { functionCall: { "@call": "other", args: {} } } };
  assert.equal(localAction(otherFn, rootScope({})), null);

  assert.deepEqual(mediaSource("data:image/png;base64,AAA"), { inline: "data:image/png;base64,AAA" });
  assert.deepEqual(mediaSource("https://example.com/x.png"), { remote: "https://example.com/x.png" });
  assert.deepEqual(mediaSource("javascript:alert(1)"), {});
  assert.deepEqual(mediaSource(""), {});
  const split = splitEventMessage('Pressed it.\n\n```a2ui-event\n{"version":"1.0"}\n```');
  assert.deepEqual(split, { text: "Pressed it.", event: true });
  assert.deepEqual(splitEventMessage("Just text."), { text: "Just text.", event: false });
});

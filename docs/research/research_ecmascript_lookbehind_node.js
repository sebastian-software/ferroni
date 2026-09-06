// PROTOTYPE — issue #44 ECMAScript look-behind research spike.
//
// This helper evaluates one case with Node so the Rust driver can compare the
// same pattern, subject, captures, and named groups with Ferroni.

const pattern = process.env.FERRONI_SPIKE_PATTERN;
const subject = process.env.FERRONI_SPIKE_SUBJECT;
const names = JSON.parse(process.env.FERRONI_SPIKE_NAMES || "[]");

try {
  const regex = new RegExp(pattern, "u");
  const match = regex.exec(subject);

  if (match === null) {
    process.stdout.write(JSON.stringify({ compile: "ok", match: null }));
  } else {
    const named = {};
    for (const name of names) {
      named[name] = match.groups?.[name] ?? null;
    }

    process.stdout.write(
      JSON.stringify({
        compile: "ok",
        match: {
          // Ferroni reports UTF-8 byte offsets while JavaScript reports UTF-16
          // code-unit offsets. Normalize Node to UTF-8 bytes for comparison.
          start: Buffer.byteLength(subject.slice(0, match.index)),
          end: Buffer.byteLength(subject.slice(0, match.index)) + Buffer.byteLength(match[0]),
          captures: Array.from(match, (capture) => capture ?? null),
          named,
        },
      }),
    );
  }
} catch (error) {
  process.stdout.write(
    JSON.stringify({
      compile: "error",
      match: null,
      detail: String(error?.message ?? error),
    }),
  );
}

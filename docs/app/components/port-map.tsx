import "./port-map.css";

/** Selected rows of the module mapping in the compatibility guide. */
const MODULES = [
  { c: "regparse.c", rust: "regparse.rs", purpose: "Pattern parser" },
  { c: "regcomp.c", rust: "regcomp.rs", purpose: "Tree to bytecode compiler" },
  { c: "regexec.c", rust: "regexec.rs", purpose: "Matching machine" },
  { c: "regsyntax.c", rust: "regsyntax.rs", purpose: "The twelve syntax modes" },
  { c: "regset.c", rust: "regset.rs", purpose: "Multi-pattern search" },
  { c: "unicode.c", rust: "unicode/", purpose: "Unicode tables, segmentation" },
  { rust: "scanner.rs", purpose: "Scanner for highlighters" },
  { rust: "api.rs", purpose: "Idiomatic Rust API" },
];

/** C source files beside the Rust modules that port them, then the modules new in Rust. */
export function PortMap() {
  return (
    <dl className="fr-port">
      {MODULES.map((module) => (
        <div key={module.rust} data-added={module.c === undefined ? "" : undefined}>
          <dt>{module.c ?? "new in Rust"}</dt>
          <dd className="fr-port-rust">{module.rust}</dd>
          <dd>{module.purpose}</dd>
        </div>
      ))}
    </dl>
  );
}

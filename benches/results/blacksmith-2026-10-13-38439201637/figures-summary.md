| Ferroni compared with | Everyday patterns | Advanced patterns | Syntax highlighting |
| --- | ---: | ---: | ---: |
| Oniguruma (C) | 2.3× faster | 1.5× faster | 12× faster |
| Shiki JS | – | – | 6.7× faster |
| Onigmo | 2.7× faster | 1.7× faster\* | 35× faster |
| PCRE2 | 1.8× faster\* | 1.6× faster\* | 29× faster |
| PCRE2 JIT | 2.0× slower\* | 3.1× slower\* | 9.9× faster |
| fancy-regex | 1.0× slower\* | 3.4× faster\* | 1.5× faster |
| regex | 1.5× slower\* | – | – |

- **Everyday patterns** (32 tasks): search and extraction in HTML, logs, chat with emoji, Markdown, JSON, CSV and source code, in syntax the regex crate also runs.
- **Advanced patterns** (17 tasks): look-around, backreferences, possessive groups, subexpression calls, absent expressions and grapheme clusters.
- **Syntax highlighting** (3 documents): the scanner calls Shiki makes for whole C, Java and PHP documents, in grammars every engine runs.

\* Ran only some of the tasks: the engine rejects a pattern or finds different matches. The figure covers the tasks it ran.

– Not measured: the engine lacks the syntax, or the multi-pattern API the workload needs.

fancy-regex is quoted in its fastest configuration per column: seek mode for everyday patterns and advanced patterns; RegexSet for syntax highlighting.

---- MODULE OlFoldStr ----
\* A string literal body: StringValue, not OpValue (2154).
EXTENDS Naturals, Sequences, SequencesExt, Bags, FiniteSetsExt
CONSTANT N
ASSUME FoldLeft(LAMBDA a, x : "s", "s", <<1>>) = "s"
====

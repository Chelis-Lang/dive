---- MODULE SequenceTailInstanceFails ----
EXTENDS Sequences
VARIABLE x
S==INSTANCE Sequences
Init==x=<<FALSE, TRUE>>
Next==UNCHANGED x
Use(F(_))==[]<>(~(F(x)=<<TRUE>>))
Live==Use(S!Tail)
====

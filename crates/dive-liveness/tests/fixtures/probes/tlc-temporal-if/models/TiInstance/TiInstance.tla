---- MODULE TiInstance ----
EXTENDS Integers
VARIABLE x
Init == x=0
Next == x'= (x+1)%3
I == INSTANCE TiTemplate WITH q <- x
Live == I!Prop
====

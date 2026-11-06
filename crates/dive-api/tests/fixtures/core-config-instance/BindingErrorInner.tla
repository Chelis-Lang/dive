---- MODULE BindingErrorInner ----
EXTENDS BindingBase, Integers
VARIABLE v
Read == 1 \div (C-v)
====

---- MODULE BindingMiddle ----
EXTENDS BindingBase, Naturals
VARIABLE w
I == INSTANCE BindingInner WITH v <- w + 1
Read == I!Read
====

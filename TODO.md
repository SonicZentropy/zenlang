# TODO - Please mark each as completed and make a git commit after each is done.  "Done" in this case means fully tested and documented as well as implemented.

Known Bug Found (not fixed)


 **Discovered known VM bugs** (documented in tour.zen header):
   - Closures with upvalue capture crash at top level (`__main__`)
   - `let mut` reassignment before `for`/`loop` at top level causes stack overflow
   - These work fine inside `fn main()

// A late chapter response must not replace newer navigation or reopen a reader
// that the user has closed (or whose owning stand has been disposed).
export function createLatestFragmentRequest(){
  let epoch=0;
  return {
    begin(){const token=++epoch;return ()=>token===epoch;},
    cancel(){epoch++;},
  };
}

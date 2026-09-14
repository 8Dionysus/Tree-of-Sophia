// Existing published research links remain reloadable during the UI migration.
const params = new URLSearchParams(location.search);
if (params.get("workspace") === "classic" || params.has("mode") || params.has("view")) {
  void import("./main");
} else {
  void import("./observatory/app.mjs").then(({mountObservatory}) => mountObservatory());
}

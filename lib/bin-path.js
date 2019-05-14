// This file exists so that it can be excluded from the V8 snapshot in Atom.
// This is required in order for the `__dirname` global to be valid.

const path = require("path");
module.exports = path.join(
  __dirname,
  "..",
  "bin",
  `notify-subprocess-${process.platform}`
);

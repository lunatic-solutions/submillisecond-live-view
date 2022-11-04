import resolve from "@rollup/plugin-node-resolve";
import commonjs from "@rollup/plugin-commonjs";
import terser from "@rollup/plugin-terser";
import injectProcessEnv from "rollup-plugin-inject-process-env";

function createExport(fileName, nodeEnv, minify) {
  return {
    input: "main.js",
    output: {
      file: `../${fileName}`,
      format: "umd",
    },
    plugins: [
      resolve(), // so Rollup can find `ms`
      commonjs(), // so Rollup can convert `ms` to an ES module
      injectProcessEnv({
        NODE_ENV: nodeEnv,
      }),
      ...(minify ? [terser()] : []),
    ],
  };
}

export default [
  // debug
  createExport("liveview-debug.js", "development", false),
  // release
  createExport("liveview-release.js", "production", true),
];

import * as cc from "./index.js";
import { createReact } from "./react-core.js";

export * from "./index.js";

export const { Code, useHighlight, useTokens, useReady } = createReact(cc);
export default Code;

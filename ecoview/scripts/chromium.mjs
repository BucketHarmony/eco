// Chromium flags shared by Playwright tests and the screenshot script.
// --use-gl=swiftshader loses the WebGL context on this machine; ANGLE's SwiftShader backend works.
export const CHROMIUM_ARGS = ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'];
export const VIEWPORT = { width: 1280, height: 800 };
// Screenshots are only comparable within one platform's SwiftShader (DECISIONS.md, Shot 6).
export const PLATFORM = `${process.platform}-${process.arch} swiftshader`;

// Chromium flags shared by Playwright tests and the screenshot script.
// --use-gl=swiftshader loses the WebGL context on this machine; ANGLE's SwiftShader backend works.
export const CHROMIUM_ARGS = ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'];
export const VIEWPORT = { width: 1280, height: 800 };

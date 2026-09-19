// The slice of pngjs the overlay tests use; pngjs ships no types, and this avoids a @types dependency.
declare module 'pngjs' {
  export const PNG: { sync: { read(buf: Buffer): { width: number; height: number; data: Buffer } } };
}

// Public hosting ships only the recorded showcase. The wallet workspace is a
// separate local build; Vite removes the unused import at build time.
if (import.meta.env.VITE_PUBLIC_PREVIEW === "true") {
  void import("./public.ts");
} else {
  void import("./main.ts");
}

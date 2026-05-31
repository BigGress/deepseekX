export default function ResizeHandle() {
  return (
    <div className="w-1 h-full cursor-col-resize flex items-center justify-center">
      <div className="w-px h-full bg-neutral-700 hover:bg-blue-500 active:bg-blue-400 transition-colors duration-150" />
    </div>
  );
}

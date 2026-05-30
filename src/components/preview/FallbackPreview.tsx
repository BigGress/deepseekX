interface FallbackPreviewProps {
  message: string;
}

export default function FallbackPreview({ message }: FallbackPreviewProps) {
  return (
    <div className="h-full flex flex-col items-center justify-center gap-2 text-neutral-500 p-8 text-center">
      <svg className="w-10 h-10 text-neutral-700" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5}
          d="M9 13h6m-3-3v6m-9 1V7a2 2 0 012-2h6l2 2h6a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z" />
      </svg>
      <p className="text-sm">{message}</p>
    </div>
  );
}

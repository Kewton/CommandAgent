"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";

export default function EditNote() {
  const router = useRouter();
  const [body, setBody] = useState("");

  return (
    <main>
      <textarea
        data-anvil-action="input"
        value={body}
        onChange={(event) => setBody(event.target.value)}
      />
      <pre>{body}</pre>
      <button data-anvil-action="primary" onClick={() => router.push("/")}>
        Save
      </button>
    </main>
  );
}

/** The browser must be able to leave its busy state when an RPC stops replying. */
export class RpcTimeoutError extends Error {
  constructor() {
    super("RPC request timed out after 15 seconds; check the connection and try again");
    this.name = "RpcTimeoutError";
  }
}

type PendingRequest<T> = {
  send(options?: Readonly<{ abortSignal?: AbortSignal }>): Promise<T>;
};

/**
 * Kit forwards abortSignal to its transport. The local deadline also bounds a
 * custom transport that does not honor cancellation; late results are ignored.
 * Submission callers still classify a timeout after broadcast as outcome unknown.
 */
export async function sendRpc<T>(request: PendingRequest<T>): Promise<T> {
  const controller = new AbortController();
  let timer: ReturnType<typeof setTimeout> | undefined;
  const deadline = new Promise<never>((_resolve, reject) => {
    timer = setTimeout(() => {
      const error = new RpcTimeoutError();
      reject(error);
      controller.abort(error);
    }, 15_000);
  });
  try {
    return await Promise.race([
      request.send({ abortSignal: controller.signal }),
      deadline,
    ]);
  } finally {
    clearTimeout(timer);
  }
}

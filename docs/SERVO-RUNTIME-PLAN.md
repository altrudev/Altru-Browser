# Servo Runtime Adapter — Next Promotion Gate

Servo 0.6.0's documented embedding model requires:

1. an `EventLoopWaker`;
2. a `ServoBuilder`;
3. a `RenderingContext`;
4. a `WebViewBuilder` with a `WebViewDelegate`;
5. repeated `Servo::spin_event_loop` calls;
6. `WebView::paint` when a new frame is ready;
7. `RenderingContext::present`;
8. image readback for offscreen verification.

For VPS verification, the preferred first runtime target is `SoftwareRenderingContext`, because Servo documents it as a consistent software-rendering path whose result can be read back with `RenderingContext::read_to_image`.

## Promotion sequence

The runtime adapter will not be enabled in normal routing until these gates pass in order:

- **R1 Compile:** Servo 0.6.0 links against our adapter feature.
- **R2 Construct:** `SoftwareRenderingContext::new` and `ServoBuilder::build` succeed.
- **R3 WebView:** a `WebViewBuilder` constructs a view with a minimal delegate.
- **R4 Local load:** a local/data document loads without external network dependency.
- **R5 Paint:** `notify_new_frame_ready` leads to `WebView::paint`.
- **R6 Readback:** a non-empty image is returned from the rendering context.
- **R7 Receipt:** frame bytes are SHA-256-bound into an AWEF execution receipt.
- **R8 Differential:** the same bounded fixture is rendered through Light and Servo and compared for text/order/geometry expectations.

No upstream Servo modification is permitted at any gate.

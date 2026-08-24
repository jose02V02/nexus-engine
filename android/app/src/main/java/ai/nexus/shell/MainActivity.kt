package ai.nexus.shell

import android.app.Activity
import android.app.AlertDialog
import android.content.ClipData
import android.content.ClipboardManager
import android.content.ComponentCallbacks2
import android.content.Context
import android.content.Intent
import android.graphics.BitmapFactory
import android.os.Bundle
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.text.Editable
import android.text.InputType
import android.text.TextWatcher
import android.view.GestureDetector
import android.view.Gravity
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import android.provider.OpenableColumns
import android.widget.ArrayAdapter
import android.widget.AutoCompleteTextView
import android.widget.Button
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.OverScroller
import android.widget.ProgressBar
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import java.io.File
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import kotlin.math.abs

class MainActivity : Activity() {
    private lateinit var address: AutoCompleteTextView
    private lateinit var pageImage: ImageView
    private lateinit var faviconView: ImageView
    private lateinit var status: TextView
    private lateinit var progress: ProgressBar
    private lateinit var backButton: Button
    private lateinit var forwardButton: Button
    private lateinit var tabsButton: Button
    private lateinit var bookmarkButton: Button
    private lateinit var newTabPanel: LinearLayout

    private val nativeExecutor: ExecutorService = Executors.newSingleThreadExecutor()
    private val mainHandler = Handler(Looper.getMainLooper())
    @Volatile private var sessionHandle: Long = 0L
    @Volatile private var tickQueued = false
    @Volatile private var suggestionGeneration = 0
    private var pendingScroll = 0f
    private var scrollRequestQueued = false
    private var privateMode = false
    private lateinit var gestureDetector: GestureDetector
    private lateinit var scaleDetector: ScaleGestureDetector
    private lateinit var overScroller: OverScroller
    private var flingLastY = 0
    private var zoomFactor = 1f
    private var pendingZoom = 1f
    private var pendingZoomX = 0f
    private var pendingZoomY = 0f
    private var zoomRequestQueued = false
    private var scalingGesture = false
    private var pendingFileMultiple = false

    private val tickRunnable = object : Runnable {
        override fun run() {
            val handle = sessionHandle
            if (handle != 0L && !tickQueued) {
                tickQueued = true
                nativeExecutor.execute {
                    try {
                        val payload = NativeBridge.tick(handle)
                        val snapshot = parseSnapshot(payload)
                        if (snapshot["dirty"] == "true") {
                            updateFrameAndSnapshot(NativeBridge.render(handle), payload, NativeBridge.favicon(handle))
                        } else {
                            applySnapshot(snapshot)
                        }
                    } finally {
                        tickQueued = false
                    }
                }
            }
            mainHandler.postDelayed(this, 100L)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        overScroller = OverScroller(this)
        gestureDetector = GestureDetector(this, object : GestureDetector.SimpleOnGestureListener() {
            override fun onDown(e: MotionEvent): Boolean {
                overScroller.abortAnimation()
                return true
            }

            override fun onSingleTapUp(e: MotionEvent): Boolean {
                if (!scalingGesture) queueTap(e.x, e.y)
                return true
            }

            override fun onScroll(e1: MotionEvent?, e2: MotionEvent, distanceX: Float, distanceY: Float): Boolean {
                if (scalingGesture) return true
                pendingScroll += distanceY
                if (kotlin.math.abs(pendingScroll) >= 6f && !scrollRequestQueued) queueScroll()
                return true
            }

            override fun onFling(e1: MotionEvent?, e2: MotionEvent, velocityX: Float, velocityY: Float): Boolean {
                if (scalingGesture) return true
                startFling((-velocityY).toInt())
                return true
            }

            override fun onLongPress(e: MotionEvent) {
                if (!scalingGesture) queueContextMenu(e.x, e.y)
            }

            override fun onDoubleTap(e: MotionEvent): Boolean {
                val target = if (zoomFactor > 1.15f) 1f else 2f
                queueZoom(target, e.x, e.y)
                return true
            }
        })
        scaleDetector = ScaleGestureDetector(this, object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
            override fun onScaleBegin(detector: ScaleGestureDetector): Boolean {
                scalingGesture = true
                pendingZoom = zoomFactor
                overScroller.abortAnimation()
                return true
            }

            override fun onScale(detector: ScaleGestureDetector): Boolean {
                pendingZoom = (pendingZoom * detector.scaleFactor).coerceIn(0.75f, 3f)
                queueZoom(pendingZoom, detector.focusX, detector.focusY)
                return true
            }

            override fun onScaleEnd(detector: ScaleGestureDetector) {
                scalingGesture = false
            }
        })
        setContentView(buildUi())
        pageImage.post { createNativeBrowser() }
        mainHandler.post(tickRunnable)
    }

    override fun onDestroy() {
        mainHandler.removeCallbacks(tickRunnable)
        val handle = sessionHandle
        sessionHandle = 0L
        if (handle != 0L) nativeExecutor.execute { NativeBridge.destroySession(handle) }
        nativeExecutor.shutdown()
        super.onDestroy()
    }

    override fun onTrimMemory(level: Int) {
        super.onTrimMemory(level)
        if (level < ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW) return
        val handle = sessionHandle
        if (handle == 0L || nativeExecutor.isShutdown) return
        val critical = level >= ComponentCallbacks2.TRIM_MEMORY_RUNNING_CRITICAL
        nativeExecutor.execute {
            runCatching { NativeBridge.notifyMemoryPressure(handle, critical) }
        }
    }

    private fun buildUi(): LinearLayout {
        val density = resources.displayMetrics.density
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding((4 * density).toInt(), (4 * density).toInt(), (4 * density).toInt(), (4 * density).toInt())
        }

        val navigation = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }
        backButton = navButton("‹") { runNativeNavigation { NativeBridge.goBack(it) } }.apply { isEnabled = false }
        forwardButton = navButton("›") { runNativeNavigation { NativeBridge.goForward(it) } }.apply { isEnabled = false }
        val reload = navButton("↻") { runNativeNavigation { NativeBridge.reload(it) } }
        tabsButton = navButton("▣ 1") { showTabs() }
        val addTab = navButton("+") { newTab() }
        val privateTab = navButton("◐") { newPrivateTab() }
        bookmarkButton = navButton("☆") { toggleBookmark() }.apply { isEnabled = false }
        val closeTab = navButton("×") { closeActiveTab() }
        val download = navButton("↓") { downloadActive() }
        val permissions = navButton("◈") { showPermissions() }
        val menu = navButton("☰") { showBrowserMenu() }
        listOf(backButton, forwardButton, reload, tabsButton, addTab, privateTab, bookmarkButton, closeTab, download, permissions, menu).forEach(navigation::addView)

        val addressRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }
        faviconView = ImageView(this).apply {
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            layoutParams = LinearLayout.LayoutParams((32 * density).toInt(), (32 * density).toInt())
        }
        address = AutoCompleteTextView(this).apply {
            setSingleLine(true)
            threshold = 1
            setText("")
            hint = "URL"
            imeOptions = EditorInfo.IME_ACTION_GO
            setOnEditorActionListener { _, actionId, _ ->
                if (actionId == EditorInfo.IME_ACTION_GO) {
                    navigateFromAddressBar(); true
                } else false
            }
            setOnItemClickListener { parent, _, position, _ ->
                val selected = parent.getItemAtPosition(position)?.toString().orEmpty()
                val url = selected.substringBefore("  —  ").trim()
                if (url.isNotEmpty()) {
                    setText(url); setSelection(text.length); navigateFromAddressBar()
                }
            }
            addTextChangedListener(object : TextWatcher {
                override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) = Unit
                override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {
                    if (hasFocus()) requestSuggestions(s?.toString().orEmpty())
                }
                override fun afterTextChanged(s: Editable?) = Unit
            })
        }
        val go = Button(this).apply { text = "GO"; setOnClickListener { navigateFromAddressBar() } }
        addressRow.addView(faviconView)
        addressRow.addView(address, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
        addressRow.addView(go)

        status = TextView(this).apply {
            text = "Nexus Engine 0.25"
            gravity = Gravity.CENTER_VERTICAL
            setPadding((6 * density).toInt(), 0, 0, 0)
        }
        progress = ProgressBar(this).apply { visibility = View.GONE }
        pageImage = ImageView(this).apply {
            scaleType = ImageView.ScaleType.FIT_XY
            setBackgroundColor(0xffffffff.toInt())
            isClickable = true
            setOnTouchListener { _, event -> handlePageTouch(event) }
        }
        newTabPanel = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding((18 * density).toInt(), (18 * density).toInt(), (18 * density).toInt(), (18 * density).toInt())
            visibility = View.VISIBLE
        }
        val newTabScroll = ScrollView(this).apply {
            visibility = View.GONE
            tag = "new-tab-scroll"
            addView(newTabPanel, ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        }
        val content = FrameLayout(this).apply {
            addView(pageImage, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
            addView(newTabScroll, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        }

        root.addView(navigation)
        root.addView(addressRow)
        root.addView(status)
        root.addView(progress, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, (3 * density).toInt()))
        root.addView(content, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))
        return root
    }

    private fun navButton(label: String, action: () -> Unit) = Button(this).apply {
        text = label
        minWidth = 0
        minimumWidth = 0
        setPadding(10, 0, 10, 0)
        setOnClickListener { action() }
    }

    private fun createNativeBrowser() {
        val width = pageImage.width.coerceAtLeast(1)
        val height = pageImage.height.coerceAtLeast(1)
        setBusy(true, "Starting Nexus Engine…")
        nativeExecutor.execute {
            val profilePath = filesDir.resolve("nexus-profile").absolutePath
            val handle = NativeBridge.createSession(width, height, profilePath)
            sessionHandle = handle
            if (handle == 0L) {
                applySnapshot(mapOf("ok" to "0", "error" to "unable to create native browser"))
                return@execute
            }
            val snapshotBytes = NativeBridge.snapshot(handle)
            val snapshot = parseSnapshot(snapshotBytes)
            if (snapshot["url"].isNullOrEmpty()) {
                updateFrameAndSnapshot(ByteArray(0), snapshotBytes, ByteArray(0))
            } else {
                updateFrameAndSnapshot(NativeBridge.render(handle), snapshotBytes, NativeBridge.favicon(handle))
            }
        }
    }

    private fun navigateFromAddressBar() {
        val input = address.text.toString().trim()
        if (input.isEmpty()) return
        setBusy(true, "Opening…")
        nativeExecutor.execute {
            val handle = sessionHandle
            val payload = if (handle != 0L) NativeBridge.navigate(handle, input) else ByteArray(0)
            updateAfterNativeOperation(payload, true)
        }
    }

    private fun runNativeNavigation(operation: (Long) -> ByteArray) {
        setBusy(true, "Loading…")
        nativeExecutor.execute {
            val handle = sessionHandle
            updateAfterNativeOperation(if (handle != 0L) operation(handle) else ByteArray(0), true)
        }
    }

    private fun newTab() {
        setBusy(true, "New tab…")
        nativeExecutor.execute {
            val handle = sessionHandle
            val payload = if (handle != 0L) NativeBridge.newTab(handle, "") else ByteArray(0)
            updateAfterNativeOperation(payload, true)
            runOnUiThread { address.setText(""); address.requestFocus(); pageImage.setImageDrawable(null) }
        }
    }

    private fun newPrivateTab() {
        setBusy(true, "Private tab…")
        nativeExecutor.execute {
            val handle = sessionHandle
            val payload = if (handle != 0L) NativeBridge.newPrivateTab(handle, "") else ByteArray(0)
            updateAfterNativeOperation(payload, true)
            runOnUiThread { address.setText(""); address.requestFocus(); pageImage.setImageDrawable(null) }
        }
    }

    private fun toggleBookmark() {
        val handle = sessionHandle
        if (handle == 0L) return
        nativeExecutor.execute {
            val payload = NativeBridge.toggleBookmark(handle)
            val snapshot = parseSnapshot(payload)
            applySnapshot(snapshot)
            runOnUiThread {
                Toast.makeText(this, if (snapshot["bookmarked"] == "true") "Aggiunto ai preferiti" else "Rimosso dai preferiti", Toast.LENGTH_SHORT).show()
                newTabPanel.removeAllViews()
            }
        }
    }

    private fun closeActiveTab() {
        setBusy(true, "Closing tab…")
        nativeExecutor.execute {
            val handle = sessionHandle
            val payload = if (handle != 0L) NativeBridge.closeActiveTab(handle) else ByteArray(0)
            updateAfterNativeOperation(payload, true)
        }
    }

    private fun showTabs() {
        val handle = sessionHandle
        if (handle == 0L) return
        nativeExecutor.execute {
            val rows = NativeBridge.tabs(handle).toString(Charsets.UTF_8)
                .lineSequence().filter { it.startsWith("tab=") }.mapNotNull { line ->
                    val parts = line.removePrefix("tab=").split('\t', limit = 6)
                    if (parts.size < 4) null else TabRow(parts[0].toLongOrNull() ?: return@mapNotNull null, parts[1] == "true", parts[2], parts[3], parts.getOrElse(4) { "normal" }, parts.getOrElse(5) { "suspended" })
                }.toList()
            runOnUiThread {
                val labels = rows.map { (if (it.active) "● " else "") + (if (it.privacy == "private") "◐ " else "") + it.title + if (it.url.isNotEmpty()) "\n${it.url} • ${it.lifecycle}" else " • ${it.lifecycle}" }.toTypedArray()
                AlertDialog.Builder(this)
                    .setTitle("Nexus Tabs (${rows.size})")
                    .setItems(labels) { _, which ->
                        val selected = rows.getOrNull(which) ?: return@setItems
                        setBusy(true, "Switching tab…")
                        nativeExecutor.execute { updateAfterNativeOperation(NativeBridge.switchTab(handle, selected.id), true) }
                    }
                    .setPositiveButton("Nuova") { _, _ -> newTab() }
                    .setNeutralButton("Privata") { _, _ -> newPrivateTab() }
                    .setNegativeButton("Chiudi", null)
                    .show()
            }
        }
    }

    private fun requestSuggestions(query: String) {
        val handle = sessionHandle
        if (handle == 0L) return
        val generation = ++suggestionGeneration
        nativeExecutor.execute {
            val values = NativeBridge.suggest(handle, query).toString(Charsets.UTF_8)
                .lineSequence().filter { it.isNotBlank() }.mapNotNull { line ->
                    val parts = line.split('\t', limit = 3)
                    if (parts.size < 3) null else "${parts[1]}  —  ${parts[2]}"
                }.toList()
            runOnUiThread {
                if (generation != suggestionGeneration || !address.hasFocus()) return@runOnUiThread
                address.setAdapter(ArrayAdapter(this, android.R.layout.simple_dropdown_item_1line, values))
                if (values.isNotEmpty()) address.showDropDown()
            }
        }
    }

    private fun downloadActive() {
        val handle = sessionHandle
        if (handle == 0L) return
        setBusy(true, "Downloading…")
        nativeExecutor.execute {
            val result = parseSnapshot(NativeBridge.downloadActive(handle))
            runOnUiThread {
                setBusy(false, "")
                if (result["ok"] == "1") Toast.makeText(this, "Saved: ${result["file"]}", Toast.LENGTH_LONG).show()
                else Toast.makeText(this, result["error"] ?: "Download failed", Toast.LENGTH_LONG).show()
            }
        }
    }


    private fun showBrowserMenu() {
        val items = arrayOf("Cronologia", "Preferiti", "Download", "Privacy Dashboard", "Impostazioni", "Cancella dati browser")
        AlertDialog.Builder(this)
            .setTitle("Nexus")
            .setItems(items) { _, which ->
                when (which) {
                    0 -> openInternalPage(0)
                    1 -> openInternalPage(1)
                    2 -> openInternalPage(2)
                    3 -> openInternalPage(4)
                    4 -> showSettingsDialog()
                    5 -> showClearDataDialog()
                }
            }
            .setNegativeButton("Chiudi", null)
            .show()
    }

    private fun openInternalPage(page: Int) {
        val handle = sessionHandle
        if (handle == 0L) return
        setBusy(true, "Opening Nexus page…")
        nativeExecutor.execute {
            val payload = NativeBridge.showInternal(handle, page)
            updateAfterNativeOperation(payload, true)
        }
    }

    private fun showSettingsDialog() {
        val handle = sessionHandle
        if (handle == 0L) return
        nativeExecutor.execute {
            val current = parseSnapshot(NativeBridge.settings(handle))
            val labels = arrayOf("JavaScript", "Ripristina sessione", "Error page offline", "Privacy dashboard")
            val keys = arrayOf("javascript_enabled", "restore_session", "offline_error_pages", "privacy_dashboard")
            val checked = BooleanArray(labels.size) { index -> current[keys[index]] == "true" }
            runOnUiThread {
                AlertDialog.Builder(this)
                    .setTitle("Impostazioni Nexus")
                    .setMultiChoiceItems(labels, checked) { _, which, isChecked ->
                        nativeExecutor.execute {
                            NativeBridge.setSetting(handle, keys[which], isChecked.toString())
                        }
                    }
                    .setNeutralButton("Zoom predefinito") { _, _ -> showDefaultZoomDialog() }
                    .setPositiveButton("Apri pagina impostazioni") { _, _ -> openInternalPage(3) }
                    .setNegativeButton("Chiudi", null)
                    .show()
            }
        }
    }

    private fun showDefaultZoomDialog() {
        val handle = sessionHandle
        if (handle == 0L) return
        val values = intArrayOf(75, 90, 100, 110, 125, 150, 200)
        val labels = values.map { "$it%" }.toTypedArray()
        nativeExecutor.execute {
            val current = parseSnapshot(NativeBridge.settings(handle))["default_zoom_percent"]?.toIntOrNull() ?: 100
            val checked = values.indices.minByOrNull { kotlin.math.abs(values[it] - current) } ?: 2
            runOnUiThread {
                AlertDialog.Builder(this)
                    .setTitle("Zoom predefinito")
                    .setSingleChoiceItems(labels, checked) { dialog, which ->
                        nativeExecutor.execute { NativeBridge.setSetting(handle, "default_zoom_percent", values[which].toString()) }
                        dialog.dismiss()
                        Toast.makeText(this, "Zoom ${values[which]}% per le nuove schede", Toast.LENGTH_SHORT).show()
                    }
                    .setNegativeButton("Annulla", null)
                    .show()
            }
        }
    }

    private fun showClearDataDialog() {
        val handle = sessionHandle
        if (handle == 0L) return
        val labels = arrayOf("Cronologia", "Cache HTTP", "localStorage", "Cookie", "Permessi", "HSTS", "Cronologia download")
        AlertDialog.Builder(this)
            .setTitle("Cancella dati browser")
            .setItems(labels) { _, which ->
                AlertDialog.Builder(this)
                    .setTitle("Conferma")
                    .setMessage("Cancellare: ${labels[which]}?")
                    .setNegativeButton("Annulla", null)
                    .setPositiveButton("Cancella") { _, _ ->
                        nativeExecutor.execute {
                            val payload = NativeBridge.clearData(handle, which)
                            applySnapshot(parseSnapshot(payload))
                            runOnUiThread { Toast.makeText(this, "${labels[which]} cancellati", Toast.LENGTH_SHORT).show() }
                        }
                    }.show()
            }
            .setNegativeButton("Chiudi", null)
            .show()
    }

    private fun showPermissions() {
        val handle = sessionHandle
        if (handle == 0L) return
        val names = arrayOf("Geolocation", "Notifications", "Camera", "Microphone", "Clipboard read", "Clipboard write")
        nativeExecutor.execute {
            val states = names.indices.map { NativeBridge.permissionState(handle, it) }
            runOnUiThread {
                val labels = names.indices.map { "${names[it]}: ${permissionLabel(states[it])}" }.toTypedArray()
                AlertDialog.Builder(this)
                    .setTitle("Site permissions")
                    .setItems(labels) { _, which -> choosePermission(handle, which, names[which]) }
                    .setNegativeButton("Chiudi", null)
                    .show()
            }
        }
    }

    private fun choosePermission(handle: Long, kind: Int, name: String) {
        val choices = arrayOf("Prompt", "Allow", "Block")
        AlertDialog.Builder(this)
            .setTitle(name)
            .setItems(choices) { _, state ->
                nativeExecutor.execute {
                    val payload = NativeBridge.setPermission(handle, kind, state)
                    applySnapshot(parseSnapshot(payload))
                }
            }.show()
    }

    private fun permissionLabel(value: Int) = when (value) { 1 -> "Allowed"; 2 -> "Blocked"; else -> "Ask" }

    private fun handlePageTouch(event: MotionEvent): Boolean {
        scaleDetector.onTouchEvent(event)
        gestureDetector.onTouchEvent(event)
        return true
    }

    private val flingRunnable = object : Runnable {
        override fun run() {
            if (!overScroller.computeScrollOffset()) return
            val current = overScroller.currY
            val delta = current - flingLastY
            flingLastY = current
            if (delta != 0) {
                pendingScroll += delta.toFloat()
                if (!scrollRequestQueued) queueScroll()
            }
            pageImage.postOnAnimation(this)
        }
    }

    private fun startFling(velocityY: Int) {
        flingLastY = 0
        overScroller.fling(0, 0, 0, velocityY, 0, 0, -1_000_000, 1_000_000)
        pageImage.removeCallbacks(flingRunnable)
        pageImage.postOnAnimation(flingRunnable)
    }

    private fun queueZoom(target: Float, focalX: Float, focalY: Float) {
        pendingZoom = target.coerceIn(0.75f, 3f)
        pendingZoomX = focalX
        pendingZoomY = focalY
        if (zoomRequestQueued) return
        zoomRequestQueued = true
        nativeExecutor.execute {
            val handle = sessionHandle
            val requested = pendingZoom
            val x = pendingZoomX
            val y = pendingZoomY
            if (handle != 0L) {
                val payload = NativeBridge.setZoom(handle, requested, x, y)
                updateFrameAndSnapshot(NativeBridge.render(handle), payload, NativeBridge.favicon(handle))
            }
            runOnUiThread {
                zoomRequestQueued = false
                if (kotlin.math.abs(pendingZoom - zoomFactor) > 0.01f) queueZoom(pendingZoom, pendingZoomX, pendingZoomY)
            }
        }
    }

    private fun queueScroll() {
        if (scrollRequestQueued) return
        val delta = pendingScroll; pendingScroll = 0f; scrollRequestQueued = true
        nativeExecutor.execute {
            val handle = sessionHandle
            if (handle != 0L) {
                NativeBridge.scrollBy(handle, delta)
                updateFrameAndSnapshot(NativeBridge.render(handle), NativeBridge.snapshot(handle), NativeBridge.favicon(handle))
            }
            runOnUiThread {
                scrollRequestQueued = false
                if (abs(pendingScroll) >= 4f) queueScroll()
            }
        }
    }

    private fun queueTap(x: Float, y: Float) {
        setBusy(true, "Interacting…")
        nativeExecutor.execute {
            val handle = sessionHandle
            val payload = if (handle != 0L) NativeBridge.tap(handle, x, y) else ByteArray(0)
            updateAfterNativeOperation(payload, true)
            maybeEditFocusedControl(payload)
        }
    }

    private fun queueContextMenu(x: Float, y: Float) {
        val handle = sessionHandle
        if (handle == 0L) return
        nativeExecutor.execute {
            val payload = NativeBridge.contextAt(handle, x, y)
            val context = parseSnapshot(payload)
            updateFrameAndSnapshot(NativeBridge.render(handle), payload, NativeBridge.favicon(handle))
            runOnUiThread { showPageContextMenu(context) }
        }
    }

    private fun showPageContextMenu(context: Map<String, String>) {
        val text = context["context_text"].orEmpty()
        val link = context["context_link"].orEmpty()
        val image = context["context_image"].orEmpty()
        val pageUrl = context["url"].orEmpty()
        val actions = mutableListOf<Pair<String, () -> Unit>>()
        if (text.isNotBlank()) actions += "Copia testo" to { copyToClipboard("Nexus text", text) }
        if (link.isNotBlank()) {
            actions += "Apri link" to { openUrl(link) }
            actions += "Apri link in nuova scheda" to { newTabWithUrl(link) }
            actions += "Copia link" to { copyToClipboard("Nexus link", link) }
        }
        if (image.isNotBlank()) {
            actions += "Apri immagine" to { openUrl(image) }
            actions += "Copia URL immagine" to { copyToClipboard("Nexus image", image) }
        }
        if (pageUrl.isNotBlank()) actions += "Copia URL pagina" to { copyToClipboard("Nexus URL", pageUrl) }
        actions += "Deseleziona" to { clearSelection() }
        AlertDialog.Builder(this)
            .setTitle(if (text.isNotBlank()) text.take(80) else "Nexus")
            .setItems(actions.map { it.first }.toTypedArray()) { _, which -> actions.getOrNull(which)?.second?.invoke() }
            .setOnDismissListener { }
            .show()
    }

    private fun copyToClipboard(label: String, value: String) {
        val clipboard = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        clipboard.setPrimaryClip(ClipData.newPlainText(label, value))
        Toast.makeText(this, "Copiato", Toast.LENGTH_SHORT).show()
    }

    private fun openUrl(url: String) {
        address.setText(url)
        address.setSelection(address.text.length)
        navigateFromAddressBar()
    }

    private fun newTabWithUrl(url: String) {
        setBusy(true, "Opening tab…")
        nativeExecutor.execute {
            val handle = sessionHandle
            val payload = if (handle != 0L) {
                if (privateMode) NativeBridge.newPrivateTab(handle, url) else NativeBridge.newTab(handle, url)
            } else ByteArray(0)
            updateAfterNativeOperation(payload, true)
        }
    }

    private fun clearSelection() {
        val handle = sessionHandle
        if (handle == 0L) return
        nativeExecutor.execute {
            val payload = NativeBridge.clearSelection(handle)
            updateFrameAndSnapshot(NativeBridge.render(handle), payload, NativeBridge.favicon(handle))
        }
    }

    private fun maybeEditFocusedControl(payload: ByteArray) {
        val snapshot = parseSnapshot(payload)
        val tag = snapshot["focused_tag"].orEmpty().lowercase()
        if (tag != "input" && tag != "textarea" && tag != "select") return
        val handle = sessionHandle
        if (handle == 0L) return
        val control = parseControlPayload(NativeBridge.focusedControl(handle)) ?: return
        if (control.disabled) return
        when {
            control.tag == "select" -> runOnUiThread { showSelectControl(control) }
            control.inputType == "file" -> runOnUiThread { openFilePicker(control) }
            control.inputType in setOf("checkbox", "radio", "hidden", "submit", "reset", "button", "image") -> Unit
            else -> runOnUiThread { showTextControl(control) }
        }
    }

    private fun showTextControl(control: FormControlUi) {
        if (control.readonly) {
            Toast.makeText(this, "Campo in sola lettura", Toast.LENGTH_SHORT).show()
            return
        }
        val editor = EditText(this).apply {
            setSingleLine(control.tag == "input" && control.inputType !in setOf("textarea"))
            hint = control.placeholder
            setText(control.value)
            setSelection(text.length)
            inputType = androidInputType(control)
            imeOptions = if (control.tag == "textarea") EditorInfo.IME_ACTION_NONE else EditorInfo.IME_ACTION_NEXT
            if (Build.VERSION.SDK_INT >= 26) {
                importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_YES
                val hints = autofillHintsFor(control)
                if (hints.isNotEmpty()) setAutofillHints(*hints)
            }
        }
        val title = buildString {
            append(if (control.name.isNotBlank()) control.name else control.inputType)
            if (control.required) append(" *")
        }
        AlertDialog.Builder(this)
            .setTitle(title)
            .setView(editor)
            .setNegativeButton("Annulla", null)
            .setNeutralButton("Invia form") { _, _ -> sendFocusedInput(editor.text.toString(), true) }
            .setPositiveButton("Applica") { _, _ -> sendFocusedInput(editor.text.toString(), false) }
            .show()
    }

    private fun showSelectControl(control: FormControlUi) {
        if (control.options.isEmpty()) return
        val labels = control.options.map { option -> if (option.disabled) "${option.label} (disabilitato)" else option.label }.toTypedArray()
        val checked = BooleanArray(control.options.size) { index -> control.options[index].selected }
        if (control.multiple) {
            AlertDialog.Builder(this)
                .setTitle(if (control.name.isBlank()) "Seleziona" else control.name)
                .setMultiChoiceItems(labels, checked) { _, which, isChecked ->
                    if (control.options[which].disabled) checked[which] = false else checked[which] = isChecked
                }
                .setNegativeButton("Annulla", null)
                .setPositiveButton("Applica") { _, _ ->
                    applySelectIndices(checked.indices.filter { checked[it] && !control.options[it].disabled })
                }.show()
        } else {
            val selected = control.options.indexOfFirst { it.selected }.coerceAtLeast(0)
            AlertDialog.Builder(this)
                .setTitle(if (control.name.isBlank()) "Seleziona" else control.name)
                .setSingleChoiceItems(labels, selected) { dialog, which ->
                    if (!control.options[which].disabled) {
                        dialog.dismiss()
                        applySelectIndices(listOf(which))
                    }
                }
                .setNegativeButton("Annulla", null).show()
        }
    }

    private fun applySelectIndices(indices: List<Int>) {
        setBusy(true, "Updating selection…")
        nativeExecutor.execute {
            val handle = sessionHandle
            if (handle == 0L) return@execute
            val payload = NativeBridge.setSelectIndices(handle, indices.joinToString(","))
            updateAfterNativeOperation(payload, true)
        }
    }

    private fun openFilePicker(control: FormControlUi) {
        pendingFileMultiple = control.multiple
        val mimeTypes = acceptMimeTypes(control.accept)
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = if (mimeTypes.size == 1) mimeTypes[0] else "*/*"
            putExtra(Intent.EXTRA_ALLOW_MULTIPLE, control.multiple)
            if (mimeTypes.size > 1) putExtra(Intent.EXTRA_MIME_TYPES, mimeTypes.toTypedArray())
        }
        @Suppress("DEPRECATION")
        startActivityForResult(intent, REQUEST_OPEN_FILE)
    }

    @Deprecated("Legacy Activity result bridge retained to avoid an AndroidX dependency in Nexus Alpha")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode != REQUEST_OPEN_FILE || resultCode != RESULT_OK || data == null) return
        val uris = mutableListOf<android.net.Uri>()
        data.clipData?.let { clip -> for (index in 0 until clip.itemCount) uris += clip.getItemAt(index).uri }
        if (uris.isEmpty()) data.data?.let(uris::add)
        if (!pendingFileMultiple && uris.size > 1) uris.subList(1, uris.size).clear()
        if (uris.isEmpty()) return
        setBusy(true, "Preparing upload…")
        nativeExecutor.execute {
            val handle = sessionHandle
            if (handle == 0L) return@execute
            var payload = NativeBridge.clearFileSelection(handle)
            var failed = false
            for ((index, uri) in uris.withIndex()) {
                try {
                    val local = copyPickedDocument(uri)
                    payload = NativeBridge.addFileSelection(handle, local.path, local.name, local.mimeType, index > 0)
                    if (parseSnapshot(payload)["ok"] == "0") { failed = true; break }
                } catch (error: Exception) {
                    payload = "ok=0\nerror=${error.message ?: "file import failed"}\n".toByteArray()
                    failed = true
                    break
                }
            }
            if (failed) NativeBridge.clearFileSelection(handle)
            updateAfterNativeOperation(payload, true)
        }
    }

    private fun copyPickedDocument(uri: android.net.Uri): PickedFile {
        var displayName = "upload.bin"
        contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
            if (cursor.moveToFirst()) displayName = cursor.getString(0) ?: displayName
        }
        displayName = displayName.replace(Regex("[\\r\\n/\\\\]"), "_").take(180).ifBlank { "upload.bin" }
        val mime = contentResolver.getType(uri) ?: "application/octet-stream"
        val dir = File(cacheDir, "nexus-upload").apply { mkdirs() }
        val target = File(dir, "${System.nanoTime()}-$displayName")
        contentResolver.openInputStream(uri).use { input ->
            requireNotNull(input) { "cannot open selected document" }
            target.outputStream().use { output ->
                val buffer = ByteArray(64 * 1024)
                var total = 0L
                while (true) {
                    val read = input.read(buffer)
                    if (read <= 0) break
                    total += read
                    if (total > 16L * 1024L * 1024L) {
                        target.delete()
                        throw IllegalArgumentException("file exceeds Nexus 16 MiB upload limit")
                    }
                    output.write(buffer, 0, read)
                }
            }
        }
        return PickedFile(target.absolutePath, displayName, mime)
    }

    private fun acceptMimeTypes(accept: String): List<String> {
        if (accept.isBlank()) return emptyList()
        val mapped = accept.split(',').mapNotNull { token ->
            val value = token.trim().lowercase()
            when {
                value.contains('/') -> value
                value == ".pdf" -> "application/pdf"
                value in setOf(".jpg", ".jpeg") -> "image/jpeg"
                value == ".png" -> "image/png"
                value == ".webp" -> "image/webp"
                value == ".txt" -> "text/plain"
                value == ".json" -> "application/json"
                else -> null
            }
        }.distinct()
        return mapped
    }

    private fun androidInputType(control: FormControlUi): Int = when (control.inputType) {
        "email" -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_WEB_EMAIL_ADDRESS
        "password" -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD
        "url" -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_URI
        "tel" -> InputType.TYPE_CLASS_PHONE
        "number", "range" -> InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_FLAG_DECIMAL or InputType.TYPE_NUMBER_FLAG_SIGNED
        "date" -> InputType.TYPE_CLASS_DATETIME or InputType.TYPE_DATETIME_VARIATION_DATE
        "time" -> InputType.TYPE_CLASS_DATETIME or InputType.TYPE_DATETIME_VARIATION_TIME
        else -> if (control.tag == "textarea") {
            InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE or InputType.TYPE_TEXT_VARIATION_LONG_MESSAGE
        } else InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_WEB_EDIT_TEXT
    }

    private fun autofillHintsFor(control: FormControlUi): Array<String> {
        val autocomplete = control.autocomplete.lowercase()
        return when {
            autocomplete.contains("email") || control.inputType == "email" -> arrayOf(View.AUTOFILL_HINT_EMAIL_ADDRESS)
            autocomplete.contains("username") -> arrayOf(View.AUTOFILL_HINT_USERNAME)
            autocomplete.contains("current-password") || control.inputType == "password" -> arrayOf(View.AUTOFILL_HINT_PASSWORD)
            autocomplete.contains("new-password") -> arrayOf("newPassword")
            autocomplete.contains("tel") || control.inputType == "tel" -> arrayOf(View.AUTOFILL_HINT_PHONE)
            autocomplete.contains("postal-code") -> arrayOf(View.AUTOFILL_HINT_POSTAL_CODE)
            autocomplete.contains("name") -> arrayOf(View.AUTOFILL_HINT_NAME)
            else -> emptyArray()
        }
    }

    private fun parseControlPayload(bytes: ByteArray): FormControlUi? {
        if (bytes.isEmpty()) return null
        val lines = bytes.toString(Charsets.UTF_8).lineSequence().toList()
        val fields = lines.filter { it.contains('=') && !it.startsWith("option=") }.associate { line ->
            val index = line.indexOf('='); line.substring(0, index) to line.substring(index + 1)
        }
        if (fields["ok"] != "1") return null
        val options = lines.filter { it.startsWith("option=") }.mapNotNull { line ->
            val parts = line.removePrefix("option=").split('\t', limit = 5)
            if (parts.size < 5) null else ControlOptionUi(parts[0].toIntOrNull() ?: return@mapNotNull null, parts[1] == "true", parts[2] == "true", parts[3], parts[4])
        }
        return FormControlUi(
            tag = fields["tag"].orEmpty(), inputType = fields["type"].orEmpty(), name = fields["name"].orEmpty(),
            value = fields["value"].orEmpty(), placeholder = fields["placeholder"].orEmpty(), autocomplete = fields["autocomplete"].orEmpty(),
            accept = fields["accept"].orEmpty(), required = fields["required"] == "true", disabled = fields["disabled"] == "true",
            readonly = fields["readonly"] == "true", checked = fields["checked"] == "true", multiple = fields["multiple"] == "true",
            min = fields["min"].orEmpty(), max = fields["max"].orEmpty(), step = fields["step"].orEmpty(), options = options
        )
    }

    private fun sendFocusedInput(value: String, submit: Boolean) {
        setBusy(true, if (submit) "Submitting…" else "Updating input…")
        nativeExecutor.execute {
            val handle = sessionHandle
            if (handle == 0L) return@execute
            var payload = NativeBridge.inputValue(handle, value)
            if (submit && parseSnapshot(payload)["ok"] != "0") payload = NativeBridge.submitFocusedForm(handle)
            updateAfterNativeOperation(payload, true)
        }
    }

    private fun updateAfterNativeOperation(payload: ByteArray, render: Boolean) {
        val snapshot = parseSnapshot(payload)
        val handle = sessionHandle
        val png = if (render && handle != 0L && snapshot["ok"] != "0") NativeBridge.render(handle) else ByteArray(0)
        val favicon = if (handle != 0L && snapshot["ok"] != "0") NativeBridge.favicon(handle) else ByteArray(0)
        if (render) updateFrameAndSnapshot(png, payload, favicon) else applySnapshot(snapshot)
    }

    private fun updateFrameAndSnapshot(png: ByteArray, payload: ByteArray, favicon: ByteArray) {
        val bitmap = if (png.isNotEmpty()) BitmapFactory.decodeByteArray(png, 0, png.size) else null
        val icon = if (favicon.isNotEmpty()) BitmapFactory.decodeByteArray(favicon, 0, favicon.size) else null
        val snapshot = parseSnapshot(payload)
        runOnUiThread {
            if (bitmap != null) pageImage.setImageBitmap(bitmap) else pageImage.setImageDrawable(null)
            if (icon != null) faviconView.setImageBitmap(icon) else faviconView.setImageDrawable(null)
            applySnapshotOnUi(snapshot)
        }
    }

    private fun applySnapshot(snapshot: Map<String, String>) = runOnUiThread { applySnapshotOnUi(snapshot) }

    private fun applySnapshotOnUi(snapshot: Map<String, String>) {
        setBusy(false, "")
        val url = snapshot["url"].orEmpty()
        if (!address.hasFocus()) address.setText(url)
        backButton.isEnabled = snapshot["can_back"] == "true"
        forwardButton.isEnabled = snapshot["can_forward"] == "true"
        val tabs = snapshot["tab_count"]?.toIntOrNull() ?: 1
        tabsButton.text = "▣ $tabs"
        val previousPrivateMode = privateMode
        privateMode = snapshot["private"] == "true"
        zoomFactor = snapshot["zoom"]?.toFloatOrNull()?.coerceIn(0.75f, 3f) ?: zoomFactor
        pendingZoom = zoomFactor
        bookmarkButton.isEnabled = url.isNotEmpty() && !url.startsWith("nexus://")
        bookmarkButton.text = if (snapshot["bookmarked"] == "true") "★" else "☆"
        val newTabScroll = newTabPanel.parent as? ScrollView
        if (url.isEmpty()) {
            pageImage.visibility = View.GONE
            newTabScroll?.visibility = View.VISIBLE
            if (newTabPanel.childCount == 0 || previousPrivateMode != privateMode) {
                newTabPanel.removeAllViews()
                refreshNewTabPanel()
            }
        } else {
            pageImage.visibility = View.VISIBLE
            newTabScroll?.visibility = View.GONE
        }

        val error = snapshot["error"]
        val title = snapshot["title"].orEmpty()
        val scroll = snapshot["scroll_y"]?.toFloatOrNull()?.toInt() ?: 0
        val maxScroll = snapshot["max_scroll_y"]?.toFloatOrNull()?.toInt() ?: 0
        status.text = if (!error.isNullOrEmpty()) "Nexus: $error" else {
            val pageTitle = title.ifEmpty { if (url.isEmpty()) "New Tab" else "Nexus Engine 0.25" }
            val scripts = snapshot["js_scripts"]?.toIntOrNull() ?: 0
            val mutations = snapshot["js_mutations"]?.toIntOrNull() ?: 0
            val wsActive = snapshot["ws_active"]?.toIntOrNull() ?: 0
            val downloads = snapshot["downloads"]?.toIntOrNull() ?: 0
            val csp = if (snapshot["csp_active"] == "true") " CSP" else ""
            val privacy = if (privateMode) " ◐PRIVATE" else ""
            val zoom = (zoomFactor * 100f).toInt()
            "$pageTitle$privacy  •  T$tabs JS $scripts/$mutations W$wsActive$csp D$downloads Z$zoom%  •  $scroll/$maxScroll csspx"
        }
    }

    private fun refreshNewTabPanel() {
        val handle = sessionHandle
        if (handle == 0L) return
        nativeExecutor.execute {
            val rows = NativeBridge.newTabData(handle).toString(Charsets.UTF_8)
                .lineSequence().filter { it.isNotBlank() }.mapNotNull { line ->
                    val parts = line.split('\t', limit = 3)
                    if (parts.size < 3) null else Triple(parts[0], parts[1], parts[2])
                }.toList()
            runOnUiThread {
                newTabPanel.removeAllViews()
                val title = TextView(this).apply {
                    text = if (privateMode) "Nexus Private" else "Nexus"
                    textSize = 28f
                    setPadding(0, 8, 0, 20)
                }
                val subtitle = TextView(this).apply {
                    text = if (privateMode) "Navigazione privata: cronologia e stato non verranno salvati." else "Nuova scheda • Preferiti e pagine recenti"
                    textSize = 15f
                    setPadding(0, 0, 0, 20)
                }
                newTabPanel.addView(title)
                newTabPanel.addView(subtitle)
                if (rows.isEmpty()) {
                    newTabPanel.addView(TextView(this).apply { text = "Nessun preferito o pagina recente. Usa la barra in alto per iniziare." })
                } else {
                    rows.forEach { row ->
                        val button = Button(this).apply {
                            text = (if (row.first == "bookmark") "★ " else "↗ ") + row.third + "\n" + row.second
                            gravity = Gravity.START or Gravity.CENTER_VERTICAL
                            setOnClickListener {
                                address.setText(row.second); address.setSelection(address.text.length); navigateFromAddressBar()
                            }
                        }
                        newTabPanel.addView(button, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
                    }
                }
            }
        }
    }

    @Suppress("DEPRECATION")
    override fun onBackPressed() {
        if (address.hasFocus()) {
            address.clearFocus()
            return
        }
        val handle = sessionHandle
        if (handle == 0L) {
            finish()
            return
        }
        nativeExecutor.execute {
            val snapshot = parseSnapshot(NativeBridge.snapshot(handle))
            when {
                snapshot["selected_text"].orEmpty().isNotEmpty() -> {
                    val payload = NativeBridge.clearSelection(handle)
                    updateFrameAndSnapshot(NativeBridge.render(handle), payload, NativeBridge.favicon(handle))
                }
                snapshot["can_back"] == "true" -> updateAfterNativeOperation(NativeBridge.goBack(handle), true)
                (snapshot["tab_count"]?.toIntOrNull() ?: 1) > 1 -> updateAfterNativeOperation(NativeBridge.closeActiveTab(handle), true)
                else -> runOnUiThread { finish() }
            }
        }
    }

    private fun parseSnapshot(bytes: ByteArray): Map<String, String> {
        if (bytes.isEmpty()) return mapOf("ok" to "0", "error" to "native operation failed")
        return bytes.toString(Charsets.UTF_8).lineSequence().filter { it.contains('=') }.associate { line ->
            val index = line.indexOf('='); line.substring(0, index) to line.substring(index + 1)
        }
    }

    private fun setBusy(busy: Boolean, message: String) = runOnUiThread {
        progress.visibility = if (busy) View.VISIBLE else View.GONE
        if (busy && message.isNotEmpty()) status.text = message
    }

    private data class TabRow(val id: Long, val active: Boolean, val title: String, val url: String, val privacy: String, val lifecycle: String)
    private data class ControlOptionUi(val index: Int, val selected: Boolean, val disabled: Boolean, val value: String, val label: String)
    private data class FormControlUi(val tag: String, val inputType: String, val name: String, val value: String, val placeholder: String, val autocomplete: String, val accept: String, val required: Boolean, val disabled: Boolean, val readonly: Boolean, val checked: Boolean, val multiple: Boolean, val min: String, val max: String, val step: String, val options: List<ControlOptionUi>)
    private data class PickedFile(val path: String, val name: String, val mimeType: String)

    companion object { private const val REQUEST_OPEN_FILE = 1301 }
}

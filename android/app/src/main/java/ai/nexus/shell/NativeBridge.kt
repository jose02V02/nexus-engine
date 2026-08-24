package ai.nexus.shell

object NativeBridge {
    init { System.loadLibrary("nexus_android") }

    @JvmStatic external fun createSession(width: Int, height: Int, profilePath: String): Long
    @JvmStatic external fun destroySession(handle: Long)
    @JvmStatic external fun navigate(handle: Long, url: String): ByteArray
    @JvmStatic external fun reload(handle: Long): ByteArray
    @JvmStatic external fun goBack(handle: Long): ByteArray
    @JvmStatic external fun goForward(handle: Long): ByteArray
    @JvmStatic external fun tap(handle: Long, x: Float, y: Float): ByteArray
    @JvmStatic external fun inputValue(handle: Long, value: String): ByteArray
    @JvmStatic external fun focusedControl(handle: Long): ByteArray
    @JvmStatic external fun setChecked(handle: Long, checked: Boolean): ByteArray
    @JvmStatic external fun setSelectIndices(handle: Long, indicesCsv: String): ByteArray
    @JvmStatic external fun addFileSelection(handle: Long, path: String, name: String, mimeType: String, append: Boolean): ByteArray
    @JvmStatic external fun clearFileSelection(handle: Long): ByteArray
    @JvmStatic external fun submitFocusedForm(handle: Long): ByteArray
    @JvmStatic external fun tick(handle: Long): ByteArray
    @JvmStatic external fun scrollBy(handle: Long, deltaY: Float): Float
    @JvmStatic external fun setZoom(handle: Long, zoom: Float, focalX: Float, focalY: Float): ByteArray
    @JvmStatic external fun contextAt(handle: Long, x: Float, y: Float): ByteArray
    @JvmStatic external fun clearSelection(handle: Long): ByteArray
    @JvmStatic external fun render(handle: Long): ByteArray
    @JvmStatic external fun snapshot(handle: Long): ByteArray

    @JvmStatic external fun newTab(handle: Long, url: String): ByteArray
    @JvmStatic external fun newPrivateTab(handle: Long, url: String): ByteArray
    @JvmStatic external fun closeActiveTab(handle: Long): ByteArray
    @JvmStatic external fun switchTab(handle: Long, tabId: Long): ByteArray
    @JvmStatic external fun tabs(handle: Long): ByteArray
    @JvmStatic external fun favicon(handle: Long): ByteArray
    @JvmStatic external fun suggest(handle: Long, query: String): ByteArray
    @JvmStatic external fun toggleBookmark(handle: Long): ByteArray
    @JvmStatic external fun bookmarks(handle: Long): ByteArray
    @JvmStatic external fun newTabData(handle: Long): ByteArray
    @JvmStatic external fun downloadActive(handle: Long): ByteArray
    @JvmStatic external fun permissionState(handle: Long, kind: Int): Int
    @JvmStatic external fun setPermission(handle: Long, kind: Int, state: Int): ByteArray
    @JvmStatic external fun showInternal(handle: Long, page: Int): ByteArray
    @JvmStatic external fun settings(handle: Long): ByteArray
    @JvmStatic external fun setSetting(handle: Long, key: String, value: String): ByteArray
    @JvmStatic external fun clearData(handle: Long, kind: Int): ByteArray
    @JvmStatic external fun notifyMemoryPressure(handle: Long, critical: Boolean): ByteArray
}

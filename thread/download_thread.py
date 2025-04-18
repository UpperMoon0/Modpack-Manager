import asyncio
from PyQt5.QtCore import QThread, pyqtSignal
from service.modpack_service import ModpackService


class DownloadThread(QThread):
    download_finished = pyqtSignal(str)  # Signal emitted when download completes
    download_progress = pyqtSignal(int, int)  # Signal emitted during download progress

    def __init__(self, url):
        super().__init__()
        self.url = url
        self.modpack_service = ModpackService()

    def run(self):
        """
        Executes the download process in a separate thread.
        Emits progress updates and signals completion.
        """
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            file_path = loop.run_until_complete(
                self.modpack_service.download(self.url, self.emit_download_progress)
            )
            self.download_finished.emit(file_path)
        except Exception as e:
            # Log the error or handle it as necessary
            self.modpack_service.logger.error(f"Download failed: {e}")
            raise

    def emit_download_progress(self, bytes_read, total_size):
        """
        Emits the download_progress signal with progress updates.
        """
        self.download_progress.emit(bytes_read, total_size)

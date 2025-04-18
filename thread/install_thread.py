from PyQt5.QtCore import QThread, pyqtSignal
from service.modpack_service import ModpackService


class InstallThread(QThread):
    install_finished = pyqtSignal()
    install_progress = pyqtSignal(int, int)

    def __init__(self, path, downloaded_file_path):
        super().__init__()
        self.path = path
        self.downloaded_file_path = downloaded_file_path
        self.modpack_service = ModpackService()

    def run(self):
        """
        Executes the installation process in a separate thread.
        Emits progress updates and signals completion.
        """
        try:
            self.modpack_service.install(
                self.path,
                self.downloaded_file_path,
                self.emit_install_progress
            )
            self.install_finished.emit()
        except Exception as e:
            # Log the error or handle it as necessary
            self.modpack_service.logger.error(f"Installation failed: {e}")
            raise

    def emit_install_progress(self, files_extracted, total_files):
        """
        Emits the install_progress signal with progress updates.
        """
        self.install_progress.emit(files_extracted, total_files)

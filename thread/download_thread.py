import asyncio
import time

from PyQt5.QtCore import QThread, pyqtSignal

from model.url_registries import UrlRegistries
from service.jdk_service import JDKService
from service.launcher_service import LauncherService
from service.modpack_service import ModpackService


class DownloadThread(QThread):
    download_finished = pyqtSignal(str)
    download_progress = pyqtSignal(int, int)

    def __init__(self, selected_version, url_registries: UrlRegistries, do_install_launcher, do_install_jdk, launcher_path, status_label):
        super().__init__()
        self.selected_version = selected_version
        self.url_registries = url_registries
        self.do_install_launcher = do_install_launcher
        self.do_install_jdk = do_install_jdk
        self.launcher_path = launcher_path
        self.modpack_service = ModpackService()
        self.launcher_service = LauncherService()
        self.jdk_service = JDKService()
        self.status_label = status_label
        self.start_time = None
        self.elapsed_time = 0

    def run(self):
        try:
            self.start_time = time.time()
            self._download()
        except Exception as e:
            self.modpack_service.logger.error(f"Download failed: {e}")
            raise

    def _download(self):
        try:
            # Start the download for the launcher if required
            if self.do_install_launcher:
                self._download_jdk()

            if self.do_install_launcher:
                self._download_launcher()

            # Now download the modpack
            modpack_temp_path = self._download_modpack()
            self.download_finished.emit(modpack_temp_path)
        except Exception as e:
            self.modpack_service.logger.error(f"Download failed: {e}")
            raise

    def _download_jdk(self):
        try:
            self.status_label.setText("Downloading JDK...")
            self.jdk_service.download_jdk(self.url_registries, self.emit_download_progress)
        except Exception as e:
            self.status_label.setText("Failed to download JDK")
            self.jdk_service.logger.error(f"JDK download failed: {e}")
            raise

    def _download_launcher(self):
        try:
            # Download the launcher file
            self.status_label.setText("Downloading launcher...")
            self.launcher_service.download_launcher(self.url_registries, self.emit_download_progress)
        except Exception:
            self.status_label.setText("Failed to download launcher")
            self.modpack_service.logger.error(f"Launcher download failed.")
            raise

    def _download_modpack(self):
        try:
            # Download the modpack file
            self.status_label.setText("Downloading modpack...")
            modpack_url = self.url_registries.gtnh_modpack_url.get(self.selected_version)
            file_path = self.modpack_service.download_modpack(modpack_url, self.emit_download_progress)
            return file_path
        except Exception as e:
            self.status_label.setText("Failed to download modpack")
            self.modpack_service.logger.error(f"Modpack download failed: {e}")
            raise

    def emit_download_progress(self, bytes_read, total_size):
        self.elapsed_time = time.time() - self.start_time
        self.download_progress.emit(bytes_read, total_size)
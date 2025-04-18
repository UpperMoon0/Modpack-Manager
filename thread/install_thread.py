from PyQt5.QtCore import QThread, pyqtSignal

from service.jdk_service import JDKService
from service.launcher_service import LauncherService
from service.modpack_service import ModpackService


class InstallThread(QThread):
    """
    A QThread subclass responsible for managing the installation of the launcher and modpack.
    Emits progress and completion signals during the installation process.
    """
    install_finished = pyqtSignal()
    install_progress = pyqtSignal(int, int)

    def __init__(self, launcher_path, temp_modpack_path, status_label, do_install_launcher, do_install_jdk):
        """
        Initializes the InstallThread instance.

        Args:
            launcher_path (str): The path where the launcher is installed.
            temp_modpack_path (str): The temporary path where the modpack files are stored.
            status_label (QLabel): A label widget used to display installation status.
        """
        super().__init__()
        self.launcher_path = launcher_path
        self.temp_modpack_path = temp_modpack_path
        self.launcher_service = LauncherService()
        self.modpack_service = ModpackService()
        self.jdk_service = JDKService()
        self.status_label = status_label
        self.do_install_launcher = do_install_launcher
        self.do_install_jdk = do_install_jdk

    def run(self):
        try:
            self._install()
        except Exception as e:
            self.modpack_service.logger.error(f"Installation failed: {e}")
            raise

    def _install(self):
        try:
            # Start the download for the launcher if required
            if self.do_install_launcher:
                self._install_jdk()

            if self.do_install_launcher:
                self._install_launcher()

            # Now download the modpack
            self._install_modpack()
        except Exception as e:
            self.modpack_service.logger.error(f"Download failed: {e}")
            raise

    def _install_jdk(self):
        """
        Handles the installation of the JDK.

        This method sets the status label to show the installation progress, installs
        the JDK using the `JDKService`, and updates the status once the installation
        is complete. If an error occurs during the installation, it is logged and raised.

        Raises:
            Exception: If the JDK installation fails.
        """
        try:
            self.status_label.setText("Installing JDK")
            self.modpack_service.logger.info("Installing JDK")
            self.jdk_service.install_jdk()
            self.modpack_service.logger.info("JDK installed successfully")
        except Exception as e:
            self.status_label.setText("Failed to install JDK")
            self.modpack_service.logger.error(f"JDK installation failed: {e}")
            raise

    def _install_launcher(self):
        """
        Handles the installation of the launcher.

        This method sets the status label to show the installation progress, installs
        the launcher using the `LauncherService`, and updates the status once the installation
        is complete. If an error occurs during the installation, it is logged and raised.

        Raises:
            Exception: If the launcher installation fails.
        """
        try:
            self.status_label.setText("Installing launcher")
            self.modpack_service.logger.info("Installing launcher")
            self.launcher_service.install_launcher(self.launcher_path)
            self.modpack_service.logger.info("Launcher installed successfully")
        except Exception as e:
            self.status_label.setText("Failed to install launcher")
            self.modpack_service.logger.error(f"Launcher installation failed: {e}")
            raise

    def _install_modpack(self):
        """
        Handles the installation of the modpack.

        This method sets the status label to show that the modpack installation is in progress,
        installs the modpack using the `ModpackService`, and emits the progress via a signal.
        Once installation is finished, it emits a signal indicating completion.

        Args:
            modpack_path (str): The path where the modpack is installed.
            temp_modpack_path (str): The temporary directory where the modpack files are located.

        Raises:
            Exception: If the modpack installation fails.
        """
        try:
            self.status_label.setText("Installing modpack")
            self.modpack_service.logger.info("Installing modpack")
            self.modpack_service.install_modpack(
                self.launcher_path,
                self.temp_modpack_path,
                self.emit_install_progress
            )
            self.install_finished.emit()  # Emit finished signal once installation is done
            self.modpack_service.logger.info("Installation completed successfully")
        except Exception as e:
            self.status_label.setText("Failed to install modpack")
            self.modpack_service.logger.error(f"Installation failed: {e}")
            raise

    def emit_install_progress(self, files_extracted, total_files):
        """
        Emits the progress of the installation process.

        This method is used to emit progress updates during the modpack installation process.
        It takes the number of files extracted and the total number of files and emits them as a signal.

        Args:
            files_extracted (int): The number of files extracted so far.
            total_files (int): The total number of files that need to be extracted.
        """
        self.install_progress.emit(files_extracted, total_files)

from PyQt5.QtCore import Qt
from PyQt5.QtGui import QIcon
from PyQt5.QtWidgets import QApplication, QWidget, QVBoxLayout, QHBoxLayout, QLabel, QLineEdit, QPushButton, \
    QProgressBar, QCheckBox, QListWidget, QFileDialog, QMessageBox

from model.url_registries import UrlRegistries
from thread.download_thread import DownloadThread
from thread.install_thread import InstallThread


class GTNHManager(QWidget):
    """
    A PyQt5 GUI application to manage the download and installation of the GT: New Horizons modpack.
    The GUI allows users to select a modpack version, download the required launcher, and install
    the modpack to the selected path.
    """
    def __init__(self):
        """
        Initializes the GTNHManager window and its components.

        Sets up the layout, widgets, and default values for the GUI. Includes checkboxes, buttons,
        input fields, a list of available versions, and progress bars to monitor the download and
        installation processes.
        """
        super().__init__()

        self.app_version = "0.5"
        self.install_thread = None
        self.download_thread = None
        self.setWindowTitle("GTNH Manager v" + self.app_version)
        self.setWindowIcon(QIcon('icon.ico'))

        self.layout = QVBoxLayout()

        # Install JDK checkbox
        self.install_jdk_checkbox = QCheckBox("Install JDK")
        self.install_jdk_checkbox.setChecked(False)  # Not checked by default
        self.layout.addWidget(self.install_jdk_checkbox)

        # Install launcher checkbox
        self.install_launcher_checkbox = QCheckBox("Install launcher")
        self.install_launcher_checkbox.setChecked(False)  # Set checked by default
        self.layout.addWidget(self.install_launcher_checkbox)

        # MultiMC launcher path input and browse button
        self.label = QLabel("Launcher path:")
        self.layout.addWidget(self.label)

        self.path_layout = QHBoxLayout()
        self.launcher_path_field = QLineEdit("D:/Games/MultiMC")  # Prefill the launcher path
        self.path_layout.addWidget(self.launcher_path_field)

        self.browse_button = QPushButton("Browse...")
        self.browse_button.clicked.connect(self.open_file_dialog)
        self.path_layout.addWidget(self.browse_button)

        self.layout.addLayout(self.path_layout)

        # Version list
        self.version_list = QListWidget()
        self.url_registries = UrlRegistries()
        version_keys = sorted(self.url_registries.gtnh_modpack_url.keys(), reverse=True)
        self.version_list.addItems(version_keys)
        self.layout.addWidget(self.version_list)

        # Install button
        self.install_button = QPushButton("Install")
        self.install_button.clicked.connect(self.on_install_clicked)
        self.layout.addWidget(self.install_button)

        # Status label
        self.status_label = QLabel("Idle")
        self.status_label.setAlignment(Qt.AlignCenter)
        self.layout.addWidget(self.status_label)

        # Progress bar
        self.progress_bar = QProgressBar()
        self.layout.addWidget(self.progress_bar)

        # Progress label
        self.progress_label = QLabel("0 / 0 bytes, 0 bytes/s")
        self.progress_label.setAlignment(Qt.AlignCenter)
        self.progress_label.setVisible(False)  # Initially hidden
        self.layout.addWidget(self.progress_label)

        self.setLayout(self.layout)

    def open_file_dialog(self):
        """
        Opens a file dialog to select a directory for the MultiMC launcher path.

        This method allows the user to navigate their file system and choose a folder where the
        launcher should be installed. The selected path is then displayed in the text field.
        """
        directory = QFileDialog.getExistingDirectory(self, "Select MultiMC Launcher Path")
        if directory:
            self.launcher_path_field.setText(directory)

    def on_install_clicked(self):
        """
        Handles the 'Install' button click event.

        This method is triggered when the user clicks the 'Install' button. It validates that a
        modpack version is selected and the launcher path is provided. If the checks pass, it
        starts the download process by creating a DownloadThread.
        """
        selected_version = self.version_list.currentItem()
        if selected_version is None:
            QMessageBox.warning(self, "Error", "Please select the modpack version")
            return

        launcher_path = self.launcher_path_field.text().strip()
        if not launcher_path:
            QMessageBox.warning(self, "Error", "Please enter the launcher path")
            return

        self.progress_bar.setValue(0)
        self.download_thread = DownloadThread(
            selected_version.text(),
            self.url_registries,
            self.install_launcher_checkbox.isChecked(),
            self.install_jdk_checkbox.isChecked(),
            launcher_path,
            self.status_label
        )
        self.download_thread.download_finished.connect(self.on_download_finished)
        self.download_thread.download_progress.connect(self.on_download_progress)
        self.download_thread.start()

    def on_download_progress(self, bytes_read, total_size):
        """
        Updates the progress bar with the current download progress.

        This method is connected to the download progress signal emitted by the DownloadThread.
        It calculates the percentage of the download completed and updates the progress bar.

        Args:
            bytes_read (int): The number of bytes downloaded so far.
            total_size (int): The total size of the file being downloaded.
        """
        percentage = int((bytes_read / total_size * 100) if total_size != 0 else 0)  # Cast percentage to an integer, avoid division by 0
        self.progress_bar.setValue(percentage)

        # Convert bytes to megabytes
        bytes_read_mb = bytes_read / (1024 * 1024)
        total_size_mb = total_size / (1024 * 1024)

        # Calculate download speed (assuming this method is called periodically)
        download_speed = bytes_read / (self.download_thread.elapsed_time or 1)  # bytes per second
        download_speed_mb = download_speed / (1024 * 1024)  # Convert to megabytes per second

        self.progress_label.setText(f"{bytes_read_mb:.2f} / {total_size_mb:.2f} MB, {download_speed_mb:.2f} MB/s")
        self.progress_label.setVisible(True)

    def on_download_finished(self, modpack_temp_path):
        """
        Handles the completion of the download process.

        This method is called when the DownloadThread emits the 'download_finished' signal.
        It triggers the installation process by creating an InstallThread.

        Args:
            modpack_temp_path (str): The temporary path where the downloaded modpack is stored.
        """
        self.progress_bar.setValue(0)
        self.progress_label.setVisible(False)
        launcher_path = self.launcher_path_field.text().strip()
        self.install_thread = InstallThread(launcher_path, modpack_temp_path, self.status_label)
        self.install_thread.install_finished.connect(self.on_install_finished)
        self.install_thread.install_progress.connect(self.on_install_progress)
        self.install_thread.start()

    def on_install_progress(self, files_extracted, total_files):
        """
        Updates the progress bar with the current installation progress.

        This method is connected to the installation progress signal emitted by the InstallThread.
        It calculates the percentage of the installation completed and updates the progress bar.

        Args:
            files_extracted (int): The number of files extracted during the installation.
            total_files (int): The total number of files to be installed.
        """
        percentage = (int(files_extracted / total_files * 100) if total_files != 0 else 0)
        self.progress_bar.setValue(percentage)

    def on_install_finished(self):
        """
        Handles the completion of the installation process.

        This method is called when the InstallThread emits the 'install_finished' signal. It updates
        the status label to indicate that the installation is complete and sets the progress bar to 100%.
        """
        self.status_label.setText("Installation complete")
        self.progress_bar.setValue(100)

if __name__ == "__main__":
    """
    Entry point for the GTNH Manager application.

    Creates an instance of the QApplication, sets up the GTNHManager window, 
    and starts the event loop.
    """
    app = QApplication([])

    window = GTNHManager()
    window.show()

    app.exec_()
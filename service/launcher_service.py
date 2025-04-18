import logging
import os
import tempfile
import zipfile

import requests

from model.url_registries import UrlRegistries


class LauncherService:
    def __init__(self):
        self.logger = logging.getLogger(__name__)
        logging.basicConfig(
            level=logging.INFO,
            format='%(asctime)s - %(levelname)s - %(message)s',
        )

    def download_launcher(self, url_registries: UrlRegistries, progress_callback=None):
        # Get the system's temp directory and the file path
        temp_dir = tempfile.gettempdir()
        file_path = os.path.join(temp_dir, 'launcher.zip')

        url = url_registries.ulti_mc_launcher_url

        with requests.get(url, stream=True) as response:
            if response.status_code != 200:
                self.logger.error(f"Failed to download launcher: {response.status_code}")
                raise Exception(f"Failed to download launcher: {response.status_code}")

            total_size = int(response.headers.get('Content-Length', 0))
            bytes_downloaded = 0

            # Use the progress callback
            with open(file_path, 'wb') as file:
                for chunk in response.iter_content(chunk_size=1024):
                    if chunk:
                        file.write(chunk)
                        bytes_downloaded += len(chunk)

                        # Call the progress callback
                        if progress_callback:
                            progress_callback(bytes_downloaded, total_size)

        return file_path


    def install_launcher(self, launcher_path):
        if not os.path.exists(launcher_path):
            os.makedirs(launcher_path)

        temp_launcher_path = os.path.join(tempfile.gettempdir(), 'launcher.zip')

        if not zipfile.is_zipfile(temp_launcher_path):
            self.logger.error(f"Launcher file is not a zip file: {temp_launcher_path}")
            raise Exception(f"File {temp_launcher_path} is not a zip file")

        with zipfile.ZipFile(temp_launcher_path, 'r') as zip_ref:
            zip_ref.extractall(launcher_path)

        # Delete the downloaded file
        os.remove(temp_launcher_path)
        self.logger.info("Launcher installed successfully.")

